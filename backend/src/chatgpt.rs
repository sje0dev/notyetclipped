//! Official Sign in with ChatGPT plan usage. Tokens never enter application JSON responses.
use std::{os::unix::fs::OpenOptionsExt, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::state::State;

const AUTH: &str = "https://auth.openai.com/api/accounts";
const RESOURCE: &str = "https://api.openai.com/v1";
const PLAN_SCOPE: &str = "chatgpt.tokens.use.direct";

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .unwrap()
}

pub async fn checked(response: reqwest::Response) -> Result<reqwest::Response> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let request = response
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("–")
        .to_owned();
    let body: Value = response.json().await.unwrap_or_default();
    let code = body["error"]["code"]
        .as_str()
        .or(body["error"].as_str())
        .unwrap_or("request_rejected");
    // Don't persist arbitrary response bodies (or an echoed credential) in diagnostics.
    let code: String = code
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .take(100)
        .collect();
    bail!("ChatGPT HTTP {status}: {code} (Request {request})")
}

#[derive(Clone, Serialize, Deserialize)]
struct Account {
    client_id: String,
    subject: String,
    email: String,
    tokens: Option<Tokens>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Tokens {
    access_token: String,
    refresh_token: String,
    id_token: String,
    expires_at: i64,
    scope: String,
}

#[derive(Serialize, Deserialize)]
struct Pending {
    state: String,
    nonce: String,
    verifier: String,
    redirect: String,
    client_id: Option<String>,
    expires_at: i64,
}

#[derive(Serialize, Deserialize)]
struct Vault {
    host_id: String,
    accounts: Vec<Account>,
    active: Option<String>,
    pending: Option<Pending>,
}

impl Default for Vault {
    fn default() -> Self {
        Self {
            host_id: format!("urn:uuid:{}", Uuid::new_v4()),
            accounts: vec![],
            active: None,
            pending: None,
        }
    }
}

async fn load(state: &State) -> Result<(Transaction<'_, Postgres>, Vault)> {
    let mut tx = state.pool.begin().await?;
    // API and worker share the file: serialize token rotation across both processes.
    sqlx::query("SELECT pg_advisory_xact_lock(71430923)")
        .execute(&mut *tx)
        .await?;
    let vault = match tokio::fs::read(state.config.data_dir.join("auth/chatgpt.json")).await {
        Ok(bytes) => serde_json::from_slice(&bytes)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vault::default(),
        Err(e) => return Err(e.into()),
    };
    Ok((tx, vault))
}

async fn save(state: &State, vault: &Vault) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let directory = state.config.data_dir.join("auth");
    tokio::fs::create_dir_all(&directory).await?;
    tokio::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).await?;
    let path = directory.join(format!("{}.tmp", Uuid::new_v4()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true).mode(0o600);
    let mut file = tokio::fs::File::from_std(options.open(&path)?);
    tokio::io::AsyncWriteExt::write_all(&mut file, &serde_json::to_vec(vault)?).await?;
    file.sync_all().await?;
    tokio::fs::rename(path, directory.join("chatgpt.json")).await?;
    Ok(())
}

fn secret() -> String {
    URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>())
}
fn plan_allowed(tokens: &Tokens) -> bool {
    tokens.scope.split_whitespace().any(|s| s == PLAN_SCOPE)
}

pub async fn status(state: &State) -> Result<Value> {
    let (_lock, vault) = load(state).await?;
    let active = vault
        .accounts
        .iter()
        .find(|a| Some(&a.client_id) == vault.active.as_ref());
    Ok(
        json!({"connected":active.is_some_and(|a| a.tokens.is_some()),
        "plan_allowed":active.and_then(|a| a.tokens.as_ref()).is_some_and(plan_allowed),
        "active":vault.active,
        "accounts":vault.accounts.iter().map(|a| json!({"id":a.client_id,"email":a.email})).collect::<Vec<_>>() }),
    )
}

pub async fn login(state: &State, account_id: Option<&str>) -> Result<String> {
    let (lock, mut vault) = load(state).await?;
    let account = account_id
        .map(|id| {
            vault
                .accounts
                .iter()
                .find(|a| a.client_id == id)
                .context("Konto nicht gefunden.")
        })
        .transpose()?;
    let redirect = url::Url::parse(&state.config.chatgpt_callback)?;
    ensure!(
        redirect.scheme() == "http"
            && redirect.host_str() == Some("127.0.0.1")
            && redirect.query().is_none()
            && redirect.fragment().is_none(),
        "ChatGPT benötigt eine HTTP-Callback-URL auf 127.0.0.1."
    );
    let pending = Pending {
        state: secret(),
        nonce: secret(),
        verifier: secret(),
        redirect: redirect.to_string(),
        client_id: account.map(|a| a.client_id.clone()),
        expires_at: Utc::now().timestamp() + 600,
    };
    let mut url = url::Url::parse(&format!("{AUTH}/authorize"))?;
    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair(
                "client_id",
                pending
                    .client_id
                    .as_deref()
                    .unwrap_or("dynamic_agent_client"),
            )
            .append_pair("ext_agent_host_id", &vault.host_id)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", &pending.redirect)
            .append_pair(
                "scope",
                "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct",
            )
            .append_pair("resource", RESOURCE)
            .append_pair("state", &pending.state)
            .append_pair("nonce", &pending.nonce)
            .append_pair("code_challenge_method", "S256")
            .append_pair(
                "code_challenge",
                &URL_SAFE_NO_PAD.encode(Sha256::digest(pending.verifier.as_bytes())),
            );
        if let Some(account) = account {
            if let Some(tokens) = &account.tokens {
                query.append_pair("id_token_hint", &tokens.id_token);
                if !plan_allowed(tokens) {
                    query.append_pair("prompt", "consent");
                }
            }
        } else {
            query.append_pair("agent_name_hint", "NotYetClipped");
        }
    }
    vault.pending = Some(pending);
    save(state, &vault).await?;
    lock.commit().await?;
    Ok(url.to_string())
}

#[derive(Deserialize)]
pub struct Callback {
    state: String,
    code: Option<String>,
    client_id: Option<String>,
    error: Option<String>,
}

#[derive(Clone, Deserialize)]
struct Claims {
    sub: String,
    email: Option<String>,
    nonce: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    id_token: Option<String>,
    expires_in: i64,
    scope: Option<String>,
    token_type: String,
}

pub async fn callback(state: &State, callback: Callback) -> Result<()> {
    let (lock, mut vault) = load(state).await?;
    let pending = vault
        .pending
        .as_ref()
        .context("Keine laufende Anmeldung.")?;
    ensure!(
        pending.expires_at > Utc::now().timestamp() && callback.state == pending.state,
        "Anmeldung abgelaufen oder ungültiger OAuth-Status."
    );
    let pending = vault.pending.take().unwrap();
    save(state, &vault).await?; // Single-use even when exchange fails.
    ensure!(
        callback.error.is_none(),
        "Anmeldung wurde nicht freigegeben."
    );
    let issued = match (&pending.client_id, &callback.client_id) {
        (Some(saved), Some(returned)) => {
            ensure!(saved == returned, "OAuth-Client stimmt nicht überein.");
            saved.clone()
        }
        (Some(saved), None) => saved.clone(),
        (None, Some(issued)) if issued != "dynamic_agent_client" => issued.clone(),
        _ => bail!("Registrierung hat keine Client-ID zurückgegeben."),
    };
    let code = callback.code.context("Anmeldecode fehlt.")?;
    let http = client();
    let response: TokenResponse = checked(
        http.post(format!("{AUTH}/oauth/token"))
            .form(&[
                ("grant_type", "authorization_code"),
                ("client_id", &issued),
                ("code", &code),
                ("code_verifier", &pending.verifier),
                ("redirect_uri", &pending.redirect),
                ("resource", RESOURCE),
            ])
            .send()
            .await?,
    )
    .await?
    .json()
    .await?;
    ensure!(
        response.token_type.eq_ignore_ascii_case("bearer")
            && (1..=86400).contains(&response.expires_in),
        "Ungültige OAuth-Tokenantwort."
    );
    let id_token = response.id_token.context("Identitätsnachweis fehlt.")?;
    let jwks: JwkSet = checked(
        http.get("https://auth.openai.com/.well-known/jwks.json")
            .send()
            .await?,
    )
    .await?
    .json()
    .await?;
    let header = decode_header(&id_token)?;
    let key = jwks
        .find(header.kid.as_deref().context("Signaturschlüssel fehlt.")?)
        .context("Signaturschlüssel unbekannt.")?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[&issued]);
    validation.set_issuer(&["https://auth.openai.com"]);
    validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
    let claims = decode::<Claims>(&id_token, &DecodingKey::from_jwk(key)?, &validation)?.claims;
    ensure!(
        claims.nonce == pending.nonce,
        "OAuth-Nonce stimmt nicht überein."
    );
    if let Some(existing) = vault.accounts.iter().find(|a| a.client_id == issued) {
        ensure!(
            existing.subject == claims.sub,
            "Anmeldung gehört zu einem anderen Konto."
        );
    }
    let account = Account {
        client_id: issued.clone(),
        subject: claims.sub,
        email: claims.email.unwrap_or_else(|| "ChatGPT-Konto".into()),
        tokens: Some(Tokens {
            access_token: response.access_token,
            refresh_token: response
                .refresh_token
                .context("Erneuerbare Sitzung fehlt.")?,
            id_token,
            expires_at: Utc::now().timestamp() + response.expires_in,
            scope: response.scope.unwrap_or_default(),
        }),
    };
    vault.accounts.retain(|a| a.client_id != issued);
    vault.accounts.push(account);
    if vault.active.as_ref() != Some(&issued) {
        sqlx::query("UPDATE ai_control SET settings=jsonb_set(jsonb_set(settings,'{enabled}','false'),'{model}','\"\"'),paused_reason=NULL WHERE id=1").execute(&state.pool).await?;
    }
    vault.active = Some(issued);
    save(state, &vault).await?;
    lock.commit().await?;
    Ok(())
}

pub async fn access_token(state: &State) -> Result<String> {
    let (lock, mut vault) = load(state).await?;
    let account = vault
        .accounts
        .iter_mut()
        .find(|a| Some(&a.client_id) == vault.active.as_ref())
        .context("Bitte mit ChatGPT anmelden.")?;
    let tokens = account
        .tokens
        .as_mut()
        .context("Bitte erneut mit ChatGPT anmelden.")?;
    ensure!(
        plan_allowed(tokens),
        "Die Nutzung des ChatGPT-Abos wurde nicht freigegeben."
    );
    if tokens.expires_at <= Utc::now().timestamp() + 120 {
        let result = checked(
            client()
                .post(format!("{AUTH}/oauth/token"))
                .form(&[
                    ("grant_type", "refresh_token"),
                    ("client_id", &account.client_id),
                    ("refresh_token", &tokens.refresh_token),
                    ("resource", RESOURCE),
                ])
                .send()
                .await?,
        )
        .await;
        let response = match result {
            Ok(r) => r,
            Err(error) => {
                if [
                    "invalid_grant",
                    "invalid_refresh_token",
                    "token_expired",
                    "refresh_token_expired",
                    "refresh_token_invalidated",
                    "refresh_token_reused",
                ]
                .iter()
                .any(|code| error.to_string().contains(code))
                {
                    account.tokens = None;
                    save(state, &vault).await?;
                }
                return Err(error);
            }
        };
        let response: TokenResponse = response.json().await?;
        ensure!(
            response.token_type.eq_ignore_ascii_case("bearer")
                && (1..=86400).contains(&response.expires_in),
            "Ungültige OAuth-Tokenantwort."
        );
        tokens.access_token = response.access_token;
        if let Some(refresh) = response.refresh_token {
            tokens.refresh_token = refresh;
        }
        if let Some(scope) = response.scope {
            tokens.scope = scope;
        }
        // Retain the last validated ID token as a login hint only.
        tokens.expires_at = Utc::now().timestamp() + response.expires_in;
    }
    let allowed = plan_allowed(tokens);
    let token = tokens.access_token.clone();
    save(state, &vault).await?;
    lock.commit().await?;
    ensure!(
        allowed,
        "Die Nutzung des ChatGPT-Abos ist nicht mehr freigegeben."
    );
    Ok(token)
}

pub async fn models(state: &State) -> Result<Value> {
    let token = access_token(state).await?;
    let catalog: Value = checked(
        client()
            .get(format!("{RESOURCE}/models"))
            .bearer_auth(token)
            .send()
            .await?,
    )
    .await?
    .json()
    .await?;
    let models = catalog["models"]
        .as_array()
        .context("Unbekanntes Modellkatalog-Format.")?;
    Ok(json!(
        models
            .iter()
            .filter(|m| m["visibility"] == "list")
            .filter_map(|m| {
                Some(json!({"id":m["slug"].as_str()?,"name":m["display_name"].as_str()?}))
            })
            .collect::<Vec<_>>()
    ))
}

pub async fn logout(state: &State) -> Result<bool> {
    sqlx::query(
        "UPDATE ai_control SET settings=jsonb_set(settings,'{enabled}','false') WHERE id=1",
    )
    .execute(&state.pool)
    .await?;
    let (lock, mut vault) = load(state).await?;
    vault.pending = None;
    let mut revoked = true;
    if let Some(account) = vault
        .accounts
        .iter_mut()
        .find(|a| Some(&a.client_id) == vault.active.as_ref())
        && let Some(tokens) = account.tokens.take()
    {
        revoked = client()
            .post(format!("{AUTH}/oauth/revoke"))
            .form(&[
                ("token", tokens.refresh_token.as_str()),
                ("token_type_hint", "refresh_token"),
                ("client_id", &account.client_id),
            ])
            .send()
            .await
            .is_ok_and(|r| r.status().is_success());
    }
    save(state, &vault).await?;
    lock.commit().await?;
    Ok(revoked)
}
