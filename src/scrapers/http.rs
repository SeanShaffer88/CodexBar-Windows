//! Minimal HTTPS on WinHTTP (Windows) so the app needs no HTTP library
//! and no extra runtime. Off Windows it reports unsupported; the caller
//! turns that into an honest provider error.

/// GET `url` with an optional bearer token. Returns the body text.
/// Errors are plain-language and never include the token.
#[cfg(windows)]
pub fn get_json(url: &str, bearer: Option<&str>) -> Result<String, String> {
    match bearer {
        Some(tok) => {
            let auth = format!("Bearer {tok}");
            get_json_headers(url, &[("Authorization", &auth)])
        }
        None => get_json_headers(url, &[]),
    }
}

/// GET `url` with extra request headers (name, value). Same contract as
/// `get_json`.
#[cfg(windows)]
pub fn get_json_headers(url: &str, extra_headers: &[(&str, &str)]) -> Result<String, String> {
    let mut headers: Vec<(&str, &str)> = vec![("Accept", "application/json")];
    headers.extend_from_slice(extra_headers);
    let (status, body) = send("GET", url, &headers, None)?;
    if status == 401 || status == 403 {
        return Err("the server refused the API key (check it with set-key)".to_string());
    }
    if status != 200 {
        return Err(format!("the server answered with status {status}"));
    }
    Ok(body)
}

/// POST `body` to `url` with an optional bearer token and content type.
/// Returns the body text on HTTP 200; other statuses come back as
/// `status NNN` so the caller can word the error for its own service.
#[cfg(windows)]
pub fn post_json(
    url: &str,
    bearer: Option<&str>,
    content_type: &str,
    body: &str,
) -> Result<String, String> {
    let mut headers: Vec<(&str, &str)> = vec![
        ("Accept", "application/json"),
        ("Content-Type", content_type),
    ];
    let auth;
    if let Some(tok) = bearer {
        auth = format!("Bearer {tok}");
        headers.push(("Authorization", &auth));
    }
    let (status, resp) = send("POST", url, &headers, Some(body))?;
    if status != 200 {
        return Err(format!("status {status}"));
    }
    Ok(resp)
}

/// One WinHTTP request: returns (status, body text). Network-level
/// failures are plain-language errors; the status line itself is data.
#[cfg(windows)]
fn send(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
) -> Result<(u32, String), String> {
    use windows_sys::Win32::Networking::WinHttp::*;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }
    struct Handle(isize);
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe { WinHttpCloseHandle(self.0 as _) };
        }
    }

    let mut url_buf = wide(url);
    let mut comps: URL_COMPONENTS = unsafe { std::mem::zeroed() };
    comps.dwStructSize = std::mem::size_of::<URL_COMPONENTS>() as u32;
    comps.dwSchemeLength = u32::MAX;
    comps.dwHostNameLength = u32::MAX;
    comps.dwUrlPathLength = u32::MAX;
    comps.dwExtraInfoLength = u32::MAX;
    let cracked = unsafe { WinHttpCrackUrl(url_buf.as_ptr(), 0, 0, &mut comps) };
    if cracked == 0 {
        return Err("the address could not be understood".to_string());
    }
    let host = unsafe {
        String::from_utf16_lossy(std::slice::from_raw_parts(
            comps.lpszHostName,
            comps.dwHostNameLength as usize,
        ))
    };
    let mut path = unsafe {
        String::from_utf16_lossy(std::slice::from_raw_parts(
            comps.lpszUrlPath,
            comps.dwUrlPathLength as usize,
        ))
    };
    if comps.dwExtraInfoLength > 0 {
        let extra = unsafe {
            String::from_utf16_lossy(std::slice::from_raw_parts(
                comps.lpszExtraInfo,
                comps.dwExtraInfoLength as usize,
            ))
        };
        path.push_str(&extra);
    }
    let _ = &mut url_buf;

    unsafe {
        let session = Handle(WinHttpOpen(
            wide("CodexBar-Windows").as_ptr(),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            std::ptr::null(),
            std::ptr::null(),
            0,
        ) as isize);
        if session.0 == 0 {
            return Err("could not start a network session".to_string());
        }
        WinHttpSetTimeouts(session.0 as _, 10_000, 10_000, 15_000, 15_000);
        let connect =
            Handle(WinHttpConnect(session.0 as _, wide(&host).as_ptr(), comps.nPort, 0) as isize);
        if connect.0 == 0 {
            return Err(format!("could not reach {host}"));
        }
        // INTERNET_SCHEME_HTTPS == 2 in the URL_COMPONENTS scheme numbering.
        let flags = if comps.nScheme == 2 {
            WINHTTP_FLAG_SECURE
        } else {
            0
        };
        let request = Handle(WinHttpOpenRequest(
            connect.0 as _,
            wide(method).as_ptr(),
            wide(&path).as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            flags,
        ) as isize);
        if request.0 == 0 {
            return Err("could not build the request".to_string());
        }
        let mut header_text = String::new();
        for (name, value) in headers {
            header_text.push_str(&format!("{name}: {value}\r\n"));
        }
        let body_bytes = body.map(|b| b.as_bytes());
        let (data_ptr, data_len) = match &body_bytes {
            Some(b) => (b.as_ptr() as *mut _, b.len() as u32),
            None => (std::ptr::null_mut(), 0),
        };
        let sent = WinHttpSendRequest(
            request.0 as _,
            wide(&header_text).as_ptr(),
            u32::MAX,
            data_ptr,
            data_len,
            data_len,
            0,
        );
        if sent == 0 || WinHttpReceiveResponse(request.0 as _, std::ptr::null_mut()) == 0 {
            return Err(format!("no answer from {host} (offline or blocked)"));
        }
        let mut status: u32 = 0;
        let mut len: u32 = std::mem::size_of::<u32>() as u32;
        WinHttpQueryHeaders(
            request.0 as _,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            std::ptr::null(),
            &mut status as *mut u32 as *mut _,
            &mut len,
            std::ptr::null_mut(),
        );
        let mut raw: Vec<u8> = Vec::new();
        loop {
            let mut avail: u32 = 0;
            if WinHttpQueryDataAvailable(request.0 as _, &mut avail) == 0 {
                break;
            }
            if avail == 0 {
                break;
            }
            let mut buf = vec![0u8; avail as usize];
            let mut read: u32 = 0;
            if WinHttpReadData(request.0 as _, buf.as_mut_ptr() as *mut _, avail, &mut read) == 0 {
                break;
            }
            buf.truncate(read as usize);
            raw.extend_from_slice(&buf);
        }
        let text = String::from_utf8(raw).map_err(|_| "the server's answer was not text")?;
        Ok((status, text))
    }
}

#[cfg(not(windows))]
pub fn get_json(url: &str, bearer: Option<&str>) -> Result<String, String> {
    let _ = bearer;
    get_json_headers(url, &[])
}

#[cfg(not(windows))]
pub fn get_json_headers(_url: &str, _headers: &[(&str, &str)]) -> Result<String, String> {
    Err("network checks are only available in the Windows build".to_string())
}

#[cfg(not(windows))]
pub fn post_json(
    _url: &str,
    _bearer: Option<&str>,
    _content_type: &str,
    _body: &str,
) -> Result<String, String> {
    Err("network checks are only available in the Windows build".to_string())
}
