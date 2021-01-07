use futures::TryStreamExt;
use hyper::{Body, Client, Request};
use hyper_tls::HttpsConnector;
use mlua::{
    prelude::{LuaError, LuaMultiValue, LuaTable, LuaValue},
    Lua,
};
use std::{
    net::IpAddr,
    sync::{atomic::Ordering, Arc},
};
use thiserror::Error;

use super::{lib::r#async::create_future, state::SandboxState};

macro_rules! lua_error {
    ($res:expr) => {
        $res.map_err(|err| err.to_string())?;
    };
}

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

    let req_fut = client.request(req);

    let (future_reg_key, fut) = wrap_future!(state, create_future(state));

    let http_rate_limiter = sandbox_state.0.http_rate_limiter.clone();
    let sender = sandbox_state.0.async_sender.clone();
    let sandbox_state = SandboxState(sandbox_state.0.clone());

    tokio::spawn(async move {
        // Rate limit how often http calls can be made
        http_rate_limiter.until_ready().await;

        let res = req_fut.await;

        match res {
            Ok(mut res) => {
                let body = res
                    .body_mut()
                    .try_fold(Vec::new(), |mut data, chunk| async move {
                        data.extend_from_slice(&chunk);
                        Ok(data)
                    })
                    .await
                    .unwrap_or_default();

                sender
                    .send((
                        future_reg_key,
                        Some(sandbox_state),
                        Box::new(move |state| {
                            let tbl: LuaTable = lua_error!(state.create_table());
                            let headers_tbl: LuaTable = lua_error!(state.create_table());

                            for (header_name, header_value) in res.headers() {
                                lua_error!(headers_tbl.set(
                                    header_name.as_str(),
                                    lua_error!(state.create_string(&header_value.as_bytes()))
                                ));
                            }

                            lua_error!(tbl.set("headers", headers_tbl));
                            lua_error!(tbl.set("ok", res.status().is_success()));
                            lua_error!(tbl.set("redirected", res.status().is_redirection()));
                            lua_error!(tbl.set("status", res.status().as_u16()));
                            lua_error!(tbl.set("statusText", res.status().canonical_reason()));
                            lua_error!(tbl.set("url", lua_error!(state.create_string(&url))));
                            lua_error!(tbl.set("body", lua_error!(state.create_string(&body))));

                            Ok(LuaMultiValue::from_vec(vec![LuaValue::Table(tbl)]))
                        }),
                    ))
                    .unwrap();
            }
            Err(err) => {
                sender
                    .send((
                        future_reg_key,
                        Some(sandbox_state),
                        Box::new(move |_state| Err(err.to_string())),
                    ))
                    .unwrap();
            }
        }
    });

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
