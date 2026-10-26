//! Small portable guest: no filesystem, network, clock, or native imports.
use serde_json::{json, Value};
use std::cell::RefCell;
thread_local! {
    static INPUT: RefCell<Vec<u8>> = RefCell::new(vec![0; 256 * 1024]);
    static OUTPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}
#[no_mangle]
pub extern "C" fn wisp_input() -> i32 {
    INPUT.with(|input| input.borrow_mut().as_mut_ptr() as i32)
}
#[no_mangle]
pub extern "C" fn wisp_handle(length: i32) -> i64 {
    let request: Value =
        INPUT.with(|input| serde_json::from_slice(&input.borrow()[..length as usize]).unwrap());
    let response = handle(request);
    OUTPUT.with(|output| {
        let mut output = output.borrow_mut();
        *output = serde_json::to_vec(&response).unwrap();
        ((output.as_ptr() as u64) << 32 | output.len() as u64) as i64
    })
}
fn handle(request: Value) -> Value {
    let mut state = request["state"].clone();
    if !state.is_object() {
        state = json!({"title":"Our Valheim modpack","packages":"denikson-BepInExPack_Valheim-5.4.2351","revision":1,"files":[],"archives":[],"parent":null});
    }
    if !state["archives"].is_array() {
        state["archives"] = json!([]);
    }
    let event = request["event"].as_str().unwrap_or_default();
    let mut notice="Add private mod ZIPs or pin public Thunderstore dependencies. Save the draft, resolve dependencies, then share the pack or install a local profile. Private ZIPs are included in the encrypted share.".to_string();
    if event == "import" {
        let data = &request["document"];
        if data["game"] == "valheim"
            && (data["format"] == 1 || data["format"] == 2)
            && data["files"].is_array()
            && data["roots"].is_array()
        {
            state = json!({"title":data["title"],"packages":data["roots"].as_array().unwrap().iter().filter_map(Value::as_str).collect::<Vec<_>>().join("\n"),"revision":data["revision"],"files":data["files"],"archives":data["files"].as_array().unwrap().iter().filter(|f| f["source"]=="local").cloned().collect::<Vec<_>>(),"parent":request["fields"]["document_hash"]});
            notice="Imported a shared pack. Review its locked packages before installing. Editing creates a new revision; existing shared messages stay unchanged.".into();
        } else {
            notice = "This document is not a supported Valheim modpack.".into();
        }
    }
    if event == "archive_added" {
        let artifact = &request["document"]["artifact"];
        let key = artifact["package"]
            .as_str()
            .unwrap_or_default()
            .rsplit_once('-')
            .map(|v| v.0.to_lowercase());
        let mut archives = state["archives"].as_array().cloned().unwrap_or_default();
        archives.retain(|f| {
            f["package"]
                .as_str()
                .unwrap_or_default()
                .rsplit_once('-')
                .map(|v| v.0.to_lowercase())
                != key
        });
        archives.push(artifact.clone());
        state["archives"] = json!(archives);
        state["files"] = json!([]);
        state["revision"] = json!(state["revision"].as_u64().unwrap_or(0).saturating_add(1));
        notice="Private ZIP added. Resolve dependencies to lock this pack before sharing or installing.".into();
    }
    if event == "archive_removed" {
        let archives: Vec<_> = state["archives"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["sha256"] != request["fields"]["sha256"])
            .cloned()
            .collect();
        state["archives"] = json!(archives);
        state["files"] = json!([]);
        state["revision"] = json!(state["revision"].as_u64().unwrap_or(0).saturating_add(1));
    }
    if event == "action" && request["action"] == "save" {
        let title = request["fields"]["title"]
            .as_str()
            .unwrap_or("Our Valheim modpack")
            .trim();
        let packages = request["fields"]["packages"]
            .as_str()
            .unwrap_or_default()
            .trim();
        if title.is_empty() || title.len() > 120 {
            notice = "Use a pack name between 1 and 120 characters.".into();
        } else {
            state["title"] = json!(title);
            state["packages"] = json!(packages);
            state["files"] = json!([]);
            state["revision"] = json!(state["revision"].as_u64().unwrap_or(0).saturating_add(1));
        }
    }
    if event == "resolved" {
        state["files"] = request["document"].clone();
        notice="Dependencies locked and downloads verified. Share this revision or install it into your own profile folder.".into();
    }
    let roots: Vec<&str> = state["packages"]
        .as_str()
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .collect();
    let files = state["files"].as_array().cloned().unwrap_or_default();
    let rows: Vec<Value> = files
        .iter()
        .map(|f| {
            json!([
                f["package"],
                if f["source"] == "local" {
                    "Private ZIP"
                } else {
                    "Thunderstore"
                },
                format!("{} bytes", f["size"]),
                f["sha256"]
                    .as_str()
                    .unwrap_or_default()
                    .get(..12)
                    .unwrap_or_default()
            ])
        })
        .collect();
    let document = if files.is_empty() {
        Value::Null
    } else {
        json!({"format":if files.iter().any(|f| f["source"]=="local") {2} else {1},"game":"valheim","title":state["title"],"revision":state["revision"],"parent":state["parent"],"roots":roots,"files":files})
    };
    json!({"state":state,"view":{"title":"Valheim Modpacks","description":notice,"fields":[{"id":"title","label":"Pack name","value":state["title"]},{"id":"packages","label":"Thunderstore dependencies · Team-Package-1.2.3","value":state["packages"],"multiline":true}],"actions":[{"id":"save","label":"Save draft"}],"rows":rows,"document":document,"packages":roots,"files":files,"archives":state["archives"]}})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editing_invalidates_lock() {
        let start = handle(json!({"event":"open"}));
        let next = handle(
            json!({"event":"action","action":"save","state":start["state"],"fields":{"title":"Friends","packages":"Team-Mod-1.0.0"}}),
        );
        assert!(next["view"]["document"].is_null());
        assert_eq!(next["view"]["packages"], json!(["Team-Mod-1.0.0"]));
    }
    #[test]
    fn import_preserves_versions() {
        let v = handle(
            json!({"event":"import","document":{"format":1,"game":"valheim","title":"Friends","revision":4,"roots":["Team-Mod-1.0.0"],"files":[]},"fields":{"document_hash":"parent"}}),
        );
        assert_eq!(v["state"]["revision"], 4);
        assert_eq!(v["state"]["parent"], "parent");
    }
    #[test]
    fn private_archives_survive_editing_and_import_but_removal_invalidates_lock() {
        let archive =
            json!({"package":"Local-Foundation-0.1.0","sha256":"abc","size":100,"source":"local"});
        let added = handle(json!({"event":"archive_added","document":{"artifact":archive}}));
        assert_eq!(added["view"]["archives"], json!([archive]));
        let resolved =
            handle(json!({"event":"resolved","state":added["state"],"document":[archive]}));
        assert_eq!(resolved["view"]["document"]["format"], 2);
        let imported = handle(json!({"event":"import","document":resolved["view"]["document"]}));
        assert_eq!(imported["view"]["archives"], json!([archive]));
        let edited = handle(
            json!({"event":"action","action":"save","state":imported["state"],"fields":{"title":"Friends","packages":""}}),
        );
        assert_eq!(edited["view"]["archives"], json!([archive]));
        assert!(edited["view"]["document"].is_null());
        let removed = handle(
            json!({"event":"archive_removed","state":resolved["state"],"fields":{"sha256":"abc"}}),
        );
        assert_eq!(removed["view"]["archives"], json!([]));
        assert_eq!(removed["view"]["files"], json!([]));
    }
}
