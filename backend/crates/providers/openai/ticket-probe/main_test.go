package main

import (
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync"
	"testing"
)

func TestProbeMatchesSub2apiHeadersBodyAndConnection(t *testing.T) {
	for _, closeTrace := range []bool{false, true} {
		t.Run(fmt.Sprint(closeTrace), func(t *testing.T) {
			var mu sync.Mutex
			var traceRemote string
			var probeRemotes []string
			ticket := "gAAAAA" + strings.Repeat("X", 286)
			server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				mu.Lock()
				defer mu.Unlock()
				if r.URL.Path == "/cdn-cgi/trace" {
					if r.Header.Get("Authorization") != "" || r.Header.Get("Cookie") != "" || r.Header.Get("Chatgpt-Account-Id") != "" {
						t.Error("trace must not contain account credentials")
					}
					traceRemote = r.RemoteAddr
					if closeTrace {
						w.Header().Set("Connection", "close")
					}
					fmt.Fprint(w, "ip=8.8.8.8\nloc=US\n")
					return
				}
				if r.Close != true || r.ProtoMajor != 1 {
					t.Error("probe must use closing HTTP/1 connection")
				}
				if (traceRemote == r.RemoteAddr) == closeTrace {
					t.Error("unexpected connection reuse")
				}
				probeRemotes = append(probeRemotes, r.RemoteAddr)
				for key, want := range map[string]string{"User-Agent": "codex_cli_rs/0.155.1 (Ubuntu 22.4.0; x86_64) xterm-256color", "Version": "0.155.1", "Originator": "codex_cli_rs", "OpenAI-Beta": "responses=experimental", "Accept": "text/event-stream", "Content-Type": "application/json", "session_id": "test-session"} {
					if r.Header.Get(key) != want {
						t.Errorf("header mismatch: %s", key)
					}
				}
				var body map[string]any
				if json.NewDecoder(r.Body).Decode(&body) != nil || body["model"] != "gpt-6-astra" || body["stream"] != true || body["store"] != false || body["instructions"] != "Reply with exactly: pong" {
					t.Error("body mismatch")
				}
				w.Header().Set("x-codex-turn-state", ticket)
			}))
			defer server.Close()
			for i := 0; i < 2; i++ {
				input := sample(server.URL)
				output := probe(input)
				if output.Result != "success" || output.Status != 200 || output.Ticket != ticket {
					t.Error("expected valid ticket")
				}
				if (output.IP == "8.8.8.8") == closeTrace {
					t.Error("IP must be attributed only to same connection")
				}
			}
			if len(probeRemotes) != 2 || probeRemotes[0] == probeRemotes[1] {
				t.Error("attempts must not share connections")
			}
		})
	}
}

func TestTraceFailureDoesNotPreventProbe(t *testing.T) {
	for _, traceBody := range []string{"ip=127.0.0.1", "ip=10.0.0.1", "ip=bad", strings.Repeat("x", 4097)} {
		server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			if r.URL.Path == "/cdn-cgi/trace" {
				fmt.Fprint(w, traceBody)
				return
			}
			w.Header().Set("x-codex-turn-state", "gAAAAA"+strings.Repeat("X", 306))
		}))
		output := probe(sample(server.URL))
		server.Close()
		if output.Status != 200 || output.Result != "invalid_ticket" || output.IP != "" || len(output.Ticket) != 312 {
			t.Error("invalid trace must remain unknown without blocking probe")
		}
	}
}

func sample(endpoint string) request {
	return request{Endpoint: endpoint + "/codex/responses", ProxyURL: endpoint,
		Headers: map[string]string{"Authorization": "Bearer test", "Chatgpt-Account-Id": "test-account", "User-Agent": "codex_cli_rs/0.155.1 (Ubuntu 22.4.0; x86_64) xterm-256color", "Version": "0.155.1", "Originator": "codex_cli_rs", "OpenAI-Beta": "responses=experimental", "Accept": "text/event-stream", "Content-Type": "application/json", "session_id": "test-session"},
		Body:    json.RawMessage(`{"model":"gpt-6-astra","store":false,"stream":true,"instructions":"Reply with exactly: pong","input":[{"role":"user","content":[{"type":"input_text","text":"ping"}]}]}`)}
}
