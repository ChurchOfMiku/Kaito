use crossbeam::channel::Sender;
use futures::{StreamExt, TryStreamExt};
use hyper::{body::Bytes, Body, Client, Request, Response, Method};
use hyper_tls::HttpsConnector;
use mlua::{
    prelude::{LuaError, LuaMultiValue, LuaTable},
    Lua, Table, Value, String as LuaString
};
use std::{
    net::IpAddr,
    sync::{atomic::Ordering, Arc},
    convert::TryFrom
};
use thiserror::Error;

use super::{state::{LuaAsyncCallback, SandboxState}, trust::Trust};

pub fn http_fetch<'a>(
    state: &'a Lua,
    sandbox_state: &SandboxState,
    url: &str,
    options: LuaTable<'a>,
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
    let req = Request::builder().uri(url.clone());
    let req = if state.is_in_trusted_context() {
        // Method
        let mut req = req.method(options.get("method").ok().and_then(|method_str: LuaString| Method::try_from(method_str.as_bytes()).ok()).unwrap_or(Method::GET));

        // Headers
        if let Some(headers) = options.get::<_, LuaTable>("headers").ok() {
            for pair in headers.pairs::<LuaString, LuaString>() {
                if let Ok((key, value)) = pair {
                    req = req.header(key.as_bytes(), value.as_bytes());
                }
            }
        }

        // Body
        req.body(options.get::<_, LuaString>("body").map(|str| Body::from(str.as_bytes().to_vec())).unwrap_or_else(|_| Body::empty()))
    } else {
        // Untrusted users can only make empty GET requests
        req.method(Method::GET).body(Body::empty())
    };

    let req = match req {
        Ok(req) => req,
        Err(err) => {
            return Err(LuaError::ExternalError(Arc::new(
                HttpError::ErrorBuildingRequest(err.to_string()),
            )));
        }
    };

    let http_rate_limiter = sandbox_state.0.http_rate_limiter.clone();
    let sender = sandbox_state.0.async_sender.clone();

    let max_size = 1024 * 1024 * 4; // Max 4MB
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
                        .map_err(|e: hyper::Error| e.into())
                        .try_fold(Vec::new(), |mut data, chunk| async move {
                            data.extend_from_slice(&chunk);

                            if data.len() > max_size {
                                return Err(anyhow::anyhow!("max body size limit reached")).into();
                            }

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

// bot state only
pub fn lib_http(state: &Lua, sender: Sender<LuaAsyncCallback>) -> anyhow::Result<()> {
    let http = state.create_table()?;

    // http.fetch
    let sender2 = sender.clone();
    let http_fetch = state.create_function(move |state, (url, options): (String, Table)| {
        let sender = sender2.clone();

        let url = match url::Url::parse(&url) {
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

        let addrs = match url.socket_addrs(|| Some(if url.scheme() == "https" { 443 } else { 80 }))
        {
            Ok(addrs) => addrs,
            Err(err) => {
                return Err(LuaError::ExternalError(Arc::new(
                    HttpError::ErrorResolvingHosts(err.to_string()),
                )));
            }
        };

        let disallowed_addr = addrs.iter().find(|addr| {
            let ip = addr.ip();
            ip.is_multicast()
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
        let req = match Request::builder().method("GET").uri(url.clone()).body(
            if let Ok(body) = options.get::<&str, String>("body") {
                body.into()
            } else {
                Body::empty()
            },
        ) {
            Ok(req) => req,
            Err(err) => {
                return Err(LuaError::ExternalError(Arc::new(
                    HttpError::ErrorBuildingRequest(err.to_string()),
                )));
            }
        };

        let max_size = 1024 * 1024 * 4; // Max 4MB
        let fut = if options.get::<&str, bool>("stream").unwrap_or(false) {
            create_lua_future!(
                state,
                sender,
                (max_size, url, sender.clone()),
                async move {
                    match client.request(req).await {
                        Ok(res) => Ok(res),
                        Err(err) => Err(err),
                    }
                },
                |state,
                 data: (usize, String, Sender<LuaAsyncCallback>),
                 res: Result<Response<Body>, hyper::Error>| {
                    let res = res?;
                    let (max_size, url, sender) = data;

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

                    fn create_next_body(state: &Lua, sender: Sender<LuaAsyncCallback>, max_size: usize, bytes_received: usize, mut body: Body) -> anyhow::Result<LuaTable> {
                        Ok(create_lua_future!(
                            state,
                            sender,
                            (max_size, bytes_received, sender),
                            async move {
                                let bytes = body.next().await;

                                (body, bytes)
                            },
                            |state, data: (usize, usize, Sender<LuaAsyncCallback>), res: (Body, Option<Result<Bytes, hyper::Error>>)| {
                                let (max_size, mut bytes_received, sender) = data;
                                if let Some(data) = res.1 {
                                    let data = data?;

                                    bytes_received += data.len();

                                    if bytes_received > max_size {
                                        return Err(anyhow::anyhow!("max body size limit reached")).into();
                                    }

                                    let tbl: LuaTable = state.create_table()?;

                                    tbl.set("body", state.create_string(&data.to_vec())?)?;
                                    tbl.set("next_body", create_next_body(state, sender, max_size, bytes_received, res.0)?)?;

                                    Ok(LuaMultiValue::from_vec(vec![Value::Table(tbl)]))
                                } else {
                                    Ok(LuaMultiValue::default())
                                }
                            }
                        ))
                    }

                    tbl.set(
                        "next_body",
                        create_next_body(state, sender, max_size, 0, res.into_body())?
                    )?;

                    Ok(tbl)
                }
            )
        } else {
            create_lua_future!(
                state,
                sender,
                (url,),
                async move {
                    match client.request(req).await {
                        Ok(mut res) => {
                            let body = res
                                .body_mut()
                                .map_err(|e: hyper::Error| e.into())
                                .try_fold(Vec::new(), |mut data, chunk| async move {
                                    data.extend_from_slice(&chunk);

                                    if data.len() > max_size {
                                        return Err(anyhow::anyhow!("max body size limit reached")).into();
                                    }

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
            )
        };

        Ok(fut)
    })?;
    http.set("fetch", http_fetch)?;

    state.globals().set("http", http)?;

    Ok(())
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
