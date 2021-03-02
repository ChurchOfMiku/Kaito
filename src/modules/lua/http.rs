use futures::TryStreamExt;
use hyper::{Body, Client, Request, Response};
use hyper_tls::HttpsConnector;
use mlua::{
    prelude::{LuaError, LuaMultiValue, LuaTable},
    Lua,
};
use std::{
    net::IpAddr,
    sync::{atomic::Ordering, Arc},
};
use thiserror::Error;

use super::state::SandboxState;

pub fn http_fetch<'a>(
    state: &'a Lua,
    sandbox_state: &SandboxState,
    url: &str,
    _options: LuaTable<'a>,
) -> Result<LuaTable<'a>, LuaError> {
    // Check http call limit
    let calls_left = sandbox_state
        .0
        .limits
        .http_calls_left
        .load(Ordering::Relaxed);

    if calls_left == 0 {
        return Err(LuaError::ExternalError(Arc::new(
            HttpError::HttpCallLimitReached,
        )));
    } else {
        sandbox_state
            .0
            .limits
            .http_calls_left
            .store(calls_left - 1, Ordering::Relaxed)
    }

    // Parse url
    let url = match url::Url::parse(url) {
        Ok(url) => url,
        Err(err) => {
            return Err(LuaError::ExternalError(Arc::new(
                HttpError::ErrorParsingUrl(err.to_string()),
            )));
        }
    };

    match url.scheme() {
        "http" | "https" => {}
        _ => {
            return Err(LuaError::ExternalError(Arc::new(HttpError::UnknownScheme(
                url.scheme().into(),
            ))));
        }
    }

    let addrs = match url.socket_addrs(|| Some(if url.scheme() == "https" { 443 } else { 80 })) {
        Ok(addrs) => addrs,
        Err(err) => {
            return Err(LuaError::ExternalError(Arc::new(
                HttpError::ErrorResolvingHosts(err.to_string()),
            )));
        }
    };

    let disallowed_addr = addrs.iter().find(|addr| {
        let ip = addr.ip();
        ip.is_loopback()
            || ip.is_multicast()
            || ip.is_unspecified()
            || (match ip {
                IpAddr::V4(ip) => match ip.octets() {
                    [10, ..] => true,
                    [172, b, ..] if b >= 16 && b <= 31 => true,
                    [192, 168, ..] => true,
                    _ => false,
                },
                IpAddr::V6(_) => false, // IPv6 should be disabled in networking
            })
    });

    if let Some(disallowed_addr) = disallowed_addr {
        return Err(LuaError::ExternalError(Arc::new(
            HttpError::DisallowedAddress(disallowed_addr.to_string()),
        )));
    }

    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, Body>(https);

    let url = url.to_string();
    let req = match Request::builder()
        .method("GET")
        .uri(url.clone())
        .body(Body::empty())
    {
        Ok(req) => req,
        Err(err) => {
            return Err(LuaError::ExternalError(Arc::new(
                HttpError::ErrorBuildingRequest(err.to_string()),
            )));
        }
    };

    let http_rate_limiter = sandbox_state.0.http_rate_limiter.clone();
    let sender = sandbox_state.0.async_sender.clone();

    let fut = create_lua_future!(
        state,
        sender,
        (url,),
        async move {
            // Rate limit how often http calls can be made
            http_rate_limiter.until_ready().await;

            match client.request(req).await {
                Ok(mut res) => {
                    let body = res
                        .body_mut()
                        .try_fold(Vec::new(), |mut data, chunk| async move {
                            data.extend_from_slice(&chunk);
                            Ok(data)
                        })
                        .await
                        .unwrap_or_default();

                    Ok((res, body))
                }
                Err(err) => Err(err),
            }
        },
        |state, data: (String,), res: Result<(Response<Body>, Vec<u8>), hyper::Error>| {
            let (res, body) = res?;
            let (url,) = data;

            let tbl: LuaTable = state.create_table()?;
            let headers_tbl: LuaTable = state.create_table()?;

            for (header_name, header_value) in res.headers() {
                headers_tbl.set(
                    header_name.as_str(),
                    state.create_string(&header_value.as_bytes())?,
                )?;
            }

            tbl.set("headers", headers_tbl)?;
            tbl.set("ok", res.status().is_success())?;
            tbl.set("redirected", res.status().is_redirection())?;
            tbl.set("status", res.status().as_u16())?;
            tbl.set("statusText", res.status().canonical_reason())?;
            tbl.set("url", state.create_string(&url)?)?;
            tbl.set("body", state.create_string(&body)?)?;

            Ok(tbl)
        }
    );

    Ok(fut)
}

#[derive(Debug, Error)]
pub enum HttpError {
    #[error("http call limit reached")]
    HttpCallLimitReached,
    #[error("error parsing url: \"{}\"", _0)]
    ErrorParsingUrl(String),
    #[error("unknown url scheme: \"{}\"", _0)]
    UnknownScheme(String),
    #[error("error resolving hosts: {}", _0)]
    ErrorResolvingHosts(String),
    #[error("local or ipv6 addresses are not allowed: \"{}\"", _0)]
    DisallowedAddress(String),
    #[error("error building request: {}", _0)]
    ErrorBuildingRequest(String),
}
