use shellumi_core::{HttpRequest, HttpResponse, HttpTransport, SyncError, SyncVault};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct State {
    pub files: BTreeMap<String, (String, SyncVault)>,
    pub next: u64,
    pub requests: Vec<HttpRequest>,
    pub lose_ack: bool,
    pub fail_upload: bool,
    pub race: Option<SyncVault>,
    pub reuse_id: Option<String>,
    pub incomplete: bool,
    pub page_size: usize,
}
#[derive(Clone, Default)]
pub struct Drive(pub Arc<Mutex<State>>);
impl HttpTransport for Drive {
    fn send(&self, request: HttpRequest) -> Result<HttpResponse, SyncError> {
        let mut state = self.0.lock().unwrap();
        state.requests.push(request.clone());
        if request.method == "GET" && request.url.contains("generateIds?") {
            state.next += 1;
            let id = state
                .reuse_id
                .take()
                .unwrap_or_else(|| format!("file-{}", state.next));
            return Ok(HttpResponse::new(
                200,
                serde_json::json!({"ids":[id]}).to_string(),
            ));
        }
        if request.method == "GET" && request.url.contains("spaces=appDataFolder") {
            let page = request
                .url
                .split("pageToken=")
                .nth(1)
                .and_then(|s| s.split('&').next())
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0);
            let size = if state.page_size == 0 {
                1000
            } else {
                state.page_size
            };
            let files:Vec<_>=state.files.iter().skip(page).take(size).map(|(id,(name,_))|serde_json::json!({"id":id,"name":name,"mimeType":"application/json"})).collect();
            let next = if page + size < state.files.len() {
                Some((page + size).to_string())
            } else {
                None
            };
            return Ok(HttpResponse::new(200,serde_json::json!({"files":files,"nextPageToken":next,"incompleteSearch":state.incomplete}).to_string()));
        }
        if request.method == "GET" && request.url.contains("?alt=media") {
            let id = request
                .url
                .split("/files/")
                .nth(1)
                .unwrap()
                .split('?')
                .next()
                .unwrap();
            return match state.files.get(id) {
                Some((_, vault)) => Ok(HttpResponse::new(200, vault.to_json().unwrap())),
                None => Ok(HttpResponse::new(404, "not found")),
            };
        }
        if request.method == "POST" && request.url.contains("uploadType=multipart") {
            assert!(
                request
                    .headers
                    .iter()
                    .any(|(k, v)| k.eq_ignore_ascii_case("If-None-Match") && v == "*"),
                "publication must request create-only semantics"
            );
            let body = request.body_str().unwrap();
            let metadata_start = body.find("\r\n\r\n").unwrap() + 4;
            let metadata_end = body[metadata_start..].find("\r\n--").unwrap() + metadata_start;
            let metadata: serde_json::Value =
                serde_json::from_str(&body[metadata_start..metadata_end]).unwrap();
            let marker = "Content-Type: application/json\r\n\r\n";
            let start = body.find(marker).unwrap() + marker.len();
            let end = body[start..].find("\r\n--").unwrap() + start;
            let vault = SyncVault::from_json(&body[start..end]).unwrap();
            let id = metadata["id"].as_str().unwrap().to_owned();
            let name = metadata["name"].as_str().unwrap().to_owned();
            if state.files.contains_key(&id) {
                return Ok(HttpResponse::new(409, "already created"));
            }
            if state.fail_upload {
                return Ok(HttpResponse::new(503, "unavailable"));
            }
            state.files.insert(id.clone(), (name.clone(), vault));
            if let Some(race) = state.race.take() {
                state
                    .files
                    .insert("racing-publication".into(), (name, race));
            }
            if std::mem::take(&mut state.lose_ack) {
                return Err(SyncError::Network("acknowledgment lost".into()));
            }
            return Ok(HttpResponse::new(
                200,
                serde_json::json!({"id":id}).to_string(),
            ));
        }
        panic!(
            "unexpected mutable or unknown Drive operation: {} {}",
            request.method, request.url
        );
    }
}
