use url::Url;

use provider_openai::credential::{CodexCookiePolicy, CookiePolicyError};

fn policy() -> CodexCookiePolicy {
    CodexCookiePolicy::new(["session"], ["chatgpt.com"]).expect("valid policy")
}

#[test]
fn official_policy_accepts_routing_cookies_without_broadening_custom_allowlist() {
    let origin = Url::parse("https://chatgpt.com/backend-api/codex/responses").unwrap();
    let official = CodexCookiePolicy::official().unwrap();
    for name in ["__cflb", "__oailb"] {
        assert!(official.validate_capture(&origin, None, name, "/").is_ok());
        assert!(policy().validate_capture(&origin, None, name, "/").is_err());
        assert!(
            official
                .validate_capture(&origin, Some("evil.example"), name, "/")
                .is_err()
        );
    }
    assert!(
        official
            .validate_capture(&origin, None, "unrelated", "/")
            .is_err()
    );
    assert!(!official.may_replay(&origin, "chatgpt.com", "/other", true, true));
}

#[test]
fn capture_should_reject_parent_public_suffix_outside_allowlist() {
    let error = policy()
        .validate_capture(
            &Url::parse("https://chatgpt.com/backend-api").expect("valid URL"),
            Some("com"),
            "session",
            "/",
        )
        .err()
        .expect("public suffix must be rejected");

    assert_eq!(error, CookiePolicyError::InvalidScope);
}

#[test]
fn replay_should_respect_host_only_cookie_scope() {
    let policy = policy();

    assert!(!policy.may_replay(
        &Url::parse("https://api.chatgpt.com/backend-api").expect("valid URL"),
        "chatgpt.com",
        "/",
        true,
        true,
    ));
}

#[test]
fn replay_should_respect_secure_cookie_attribute() {
    let policy = policy();

    assert!(!policy.may_replay(
        &Url::parse("http://chatgpt.com/backend-api").expect("valid URL"),
        "chatgpt.com",
        "/",
        false,
        true,
    ));
}
