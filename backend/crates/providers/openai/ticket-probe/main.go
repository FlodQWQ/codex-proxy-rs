// SPDX-License-Identifier: LGPL-3.0-only
// 独立打票传输：使用与 sub2api 相同版本的 Go net/http，不改变业务流量。
package main

import (
	"bytes"
	"context"
	"crypto/tls"
	"encoding/json"
	"io"
	"net"
	"net/http"
	"net/http/httptrace"
	"net/netip"
	"net/url"
	"os"
	"strings"
	"sync"
	"time"
)

type request struct {
	Endpoint string            `json:"endpoint"`
	ProxyURL string            `json:"proxyUrl"`
	Headers  map[string]string `json:"headers"`
	Body     json.RawMessage   `json:"body"`
}

type result struct {
	Ticket string `json:"ticket"`
	Status int    `json:"status"`
	IP     string `json:"ip"`
	Result string `json:"result"`
}

func main() {
	var input request
	decoder := json.NewDecoder(io.LimitReader(os.Stdin, 65537))
	decoder.DisallowUnknownFields()
	output := result{Result: "input_error"}
	if decoder.Decode(&input) == nil {
		output = probe(input)
	}
	// stdout 仅通过匿名管道回给父进程，票据和代理凭据不写日志。
	_ = json.NewEncoder(os.Stdout).Encode(output)
}

func probe(input request) result {
	output := result{Result: "network_error"}
	endpoint, err := url.Parse(input.Endpoint)
	if err != nil || endpoint.Hostname() == "" || endpoint.User != nil {
		return result{Result: "input_error"}
	}
	if endpoint.Scheme != "https" {
		addr, e := netip.ParseAddr(endpoint.Hostname())
		if endpoint.Scheme != "http" || e != nil || !addr.IsLoopback() {
			return result{Result: "input_error"}
		}
	}
	proxy, err := url.Parse(input.ProxyURL)
	if err != nil || proxy.Hostname() == "" {
		return result{Result: "proxy_error"}
	}
	switch proxy.Scheme {
	case "http", "https", "socks5", "socks5h":
	default:
		return result{Result: "proxy_error"}
	}
	transport := &http.Transport{
		Proxy:               http.ProxyURL(proxy),
		DialContext:         (&net.Dialer{Timeout: 10 * time.Second, KeepAlive: 30 * time.Second}).DialContext,
		TLSHandshakeTimeout: 10 * time.Second,
		ForceAttemptHTTP2:   false,
		TLSNextProto:        make(map[string]func(string, *tls.Conn) http.RoundTripper),
		MaxConnsPerHost:     1,
		MaxIdleConnsPerHost: 1,
	}
	defer transport.CloseIdleConnections()
	client := &http.Client{Transport: transport, CheckRedirect: func(*http.Request, []*http.Request) error {
		return http.ErrUseLastResponse
	}}
	ctx, cancel := context.WithTimeout(context.Background(), 25*time.Second)
	defer cancel()
	var mu sync.Mutex
	var traceConn, probeConn net.Conn
	traceURL := *endpoint
	traceURL.Path, traceURL.RawPath, traceURL.RawQuery, traceURL.Fragment = "/cdn-cgi/trace", "", "", ""
	traceCtx, traceCancel := context.WithTimeout(ctx, 5*time.Second)
	defer traceCancel()
	traceCtx = httptrace.WithClientTrace(traceCtx, &httptrace.ClientTrace{GotConn: func(info httptrace.GotConnInfo) {
		mu.Lock()
		traceConn = info.Conn
		mu.Unlock()
	}})
	traceReq, _ := http.NewRequestWithContext(traceCtx, http.MethodGet, traceURL.String(), nil)
	traceReq.Header.Set("Accept", "text/plain")
	traceReq.Header.Set("Cache-Control", "no-cache")
	ip := ""
	if response, e := client.Do(traceReq); e == nil {
		body, readErr := io.ReadAll(io.LimitReader(response.Body, 4097))
		_ = response.Body.Close()
		if response.StatusCode == 200 && readErr == nil && len(body) <= 4096 {
			for _, line := range strings.Split(string(body), "\n") {
				if value, ok := strings.CutPrefix(line, "ip="); ok {
					addr, parseErr := netip.ParseAddr(strings.TrimSpace(value))
					if parseErr == nil && addr.IsGlobalUnicast() && !addr.IsPrivate() {
						ip = addr.Unmap().String()
					}
					break
				}
			}
		}
	}
	ctx = httptrace.WithClientTrace(ctx, &httptrace.ClientTrace{GotConn: func(info httptrace.GotConnInfo) {
		mu.Lock()
		probeConn = info.Conn
		mu.Unlock()
	}})
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, input.Endpoint, bytes.NewReader(input.Body))
	if err != nil {
		return result{Result: "input_error"}
	}
	req.Close = true
	for key, value := range input.Headers {
		req.Header.Set(key, value)
	}
	response, err := client.Do(req)
	mu.Lock()
	if traceConn != nil && traceConn == probeConn {
		output.IP = ip
	}
	mu.Unlock()
	if err != nil {
		return output
	}
	defer response.Body.Close()
	output.Status = response.StatusCode
	output.Ticket = strings.TrimSpace(response.Header.Get("x-codex-turn-state"))
	output.Result = "invalid_ticket"
	if output.Status != 200 {
		output.Result = "http_error"
	} else if len(output.Ticket) == 292 && strings.HasPrefix(output.Ticket, "gAAAAA") {
		output.Result = "success"
	}
	return output
}
