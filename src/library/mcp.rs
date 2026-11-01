//! The media library's MCP tools, the same in every app's server: what PolishCast, TrackCut and
//! PublishKit made and where it stands, the calendar of it, and ideas planned ahead. An app adds
//! [`tools`] to its list, tries [`call`] before its own tools, and appends [`INSTRUCTIONS`].

use super::{Item, Kind, Status};
use serde_json::{json, Value};
use std::path::PathBuf;

/// What a client is told about the tools, after the app's own instructions.
pub const INSTRUCTIONS: &str = "The media library is shared by PolishCast, TrackCut and PublishKit: every take, \
recording and post, with its status (planned, recording, polishing, ready, queued, posted, published), files, \
tags and the drafts it went into. list_media searches it, get_media reads one item, calendar gives what is \
planned, scheduled or done day by day. plan_media puts an idea on the calendar before anything is made, with \
its outline or script as notes; fulfil_plan hands it to the take, recording or post made for it. \
set_media_status, set_media_tags and set_media_notes change an item; a post's status is PublishKit's own. \
add_media adds video, audio or picture files where they are.";

pub fn tools() -> Vec<Value> {
    let id =
        json!({ "type": "string", "description": "The item's id, from list_media or calendar." });
    let statuses: Vec<&str> = Status::ALL.iter().map(|s| s.key()).collect();
    let kinds = ["video", "audio", "image", "blog", "social"];
    vec![
        json!({
            "name": "list_media",
            "description": "Items in the media library every Mayorana app shares, newest first: takes from \
                            PolishCast, recordings from TrackCut, posts from PublishKit, files added by hand and \
                            planned ideas. Each with its status, day, files, tags and the drafts it is in.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Words that must all be in its title, tags, kind, status, network or file names." },
                    "kind": { "type": "string", "enum": kinds },
                    "status": { "type": "string", "enum": statuses },
                    "app": { "type": "string", "enum": ["polishcast", "trackcut", "publishkit", "library"] },
                    "limit": { "type": "integer", "description": "Default 50." },
                },
            },
        }),
        json!({
            "name": "get_media",
            "description": "One item of the media library, with its notes and which of its files are missing.",
            "inputSchema": { "type": "object", "properties": { "id": id }, "required": ["id"] },
        }),
        json!({
            "name": "calendar",
            "description": "What is planned, scheduled, made or posted each day from `from` to `to`, across the \
                            three apps: ideas, takes and recordings on the day their status last changed, posts on \
                            their planned or posted date.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "from": { "type": "string", "description": "A day, 2026-11-02. Default today." },
                    "to": { "type": "string", "description": "A day. Default a week after from." },
                },
            },
        }),
        json!({
            "name": "plan_media",
            "description": "Puts an idea on the calendar before it is made: a video or audio to record, a post to \
                            write. Notes hold its outline or the demo's script.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "kind": { "type": "string", "enum": kinds, "description": "Default video." },
                    "date": { "type": "string", "description": "When it should be made or go out: 2026-11-04, or with a time, 2026-11-04T09:00." },
                    "notes": { "type": "string" },
                    "tags": { "type": "array", "items": { "type": "string" } },
                },
                "required": ["title", "date"],
            },
        }),
        json!({
            "name": "fulfil_plan",
            "description": "Hands a planned idea to what was made for it (a take, a recording, a post): its notes, \
                            tags and links move there, and the idea leaves the calendar.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "plan": { "type": "string", "description": "The planned idea's id." },
                    "id": { "type": "string", "description": "The item made for it." },
                },
                "required": ["plan", "id"],
            },
        }),
        json!({
            "name": "set_media_status",
            "description": "Sets an item's status by hand. It stays until the work moves past it. A post's status \
                            is set by publishing or scheduling it in PublishKit.",
            "inputSchema": {
                "type": "object",
                "properties": { "id": id, "status": { "type": "string", "enum": statuses } },
                "required": ["id", "status"],
            },
        }),
        json!({
            "name": "set_media_tags",
            "description": "Replaces an item's tags.",
            "inputSchema": {
                "type": "object",
                "properties": { "id": id, "tags": { "type": "array", "items": { "type": "string" } } },
                "required": ["id", "tags"],
            },
        }),
        json!({
            "name": "set_media_notes",
            "description": "Replaces an item's notes: an outline, a script, what is left to do.",
            "inputSchema": {
                "type": "object",
                "properties": { "id": id, "notes": { "type": "string" } },
                "required": ["id", "notes"],
            },
        }),
        json!({
            "name": "add_media",
            "description": "Adds video, audio or picture files to the library where they are (nothing is copied), \
                            as ready. Files already in it are left out.",
            "inputSchema": {
                "type": "object",
                "properties": { "paths": { "type": "array", "items": { "type": "string" }, "description": "Absolute paths." } },
                "required": ["paths"],
            },
        }),
    ]
}

/// The tool's answer, or `None` when `name` isn't one of the library's tools.
pub fn call(name: &str, args: &Value) -> Option<Result<Value, String>> {
    Some(match name {
        "list_media" => Ok(list(args)),
        "get_media" => get(args),
        "calendar" => calendar(args),
        "plan_media" => plan(args),
        "fulfil_plan" => fulfil(args),
        "set_media_status" => set_status(args),
        "set_media_tags" => set_tags(args),
        "set_media_notes" => set_notes(args),
        "add_media" => add(args),
        _ => return None,
    })
}

fn text<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args[key].as_str().map(str::trim).filter(|s| !s.is_empty())
}

fn required<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    text(args, key).ok_or(format!("{key} is required"))
}

fn kind_of(key: &str) -> Result<Kind, String> {
    Kind::ALL
        .into_iter()
        .find(|k| format!("{k:?}").eq_ignore_ascii_case(key))
        .ok_or(format!(
            "kind is video, audio, image, blog or social, not {key}"
        ))
}

fn tags_of(args: &Value) -> Vec<String> {
    args["tags"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn io(e: std::io::Error) -> String {
    e.to_string()
}

/// An item as the tools give it: no empty fields, notes only by [`get`].
fn brief(item: &Item) -> Value {
    let mut v = json!({
        "id": item.id,
        "title": item.title,
        "kind": format!("{:?}", item.kind).to_lowercase(),
        "app": item.app,
        "status": item.status.key(),
        "date": item.date,
        "files": item.files.iter().map(|f| json!({ "path": f.path, "role": f.role })).collect::<Vec<_>>(),
    });
    if let Some(n) = &item.network {
        v["network"] = json!(n);
    }
    if item.manual {
        v["set_by_hand"] = json!(true);
    }
    if !item.tags.is_empty() {
        v["tags"] = json!(item.tags);
    }
    if !item.links.is_empty() {
        v["in"] = json!(item
            .links
            .iter()
            .map(|l| json!({ "app": l.app, "label": l.label, "open": l.target }))
            .collect::<Vec<_>>());
    }
    if let Some(url) = &item.url {
        v["url"] = json!(url);
    }
    if !item.notes.is_empty() {
        v["has_notes"] = json!(true);
    }
    v
}

fn list(args: &Value) -> Value {
    let kind = text(args, "kind").and_then(|k| kind_of(k).ok());
    let status = text(args, "status").and_then(Status::from_key);
    let app = text(args, "app");
    let query = text(args, "query").unwrap_or("");
    let limit = args["limit"].as_u64().unwrap_or(50) as usize;
    let all = super::load_all();
    let found: Vec<&Item> = all
        .iter()
        .filter(|i| kind.is_none_or(|k| i.kind == k))
        .filter(|i| status.is_none_or(|s| i.status == s))
        .filter(|i| app.is_none_or(|a| i.app == a))
        .filter(|i| i.matches(query))
        .collect();
    json!({
        "total": found.len(),
        "items": found.iter().take(limit).map(|i| brief(i)).collect::<Vec<_>>(),
    })
}

fn get(args: &Value) -> Result<Value, String> {
    let id = required(args, "id")?;
    let item = super::get(id).ok_or(format!("no item {id} (list_media gives them)"))?;
    let mut v = brief(&item);
    v["notes"] = json!(item.notes);
    let missing: Vec<&PathBuf> = item.missing().into_iter().map(|f| &f.path).collect();
    if !missing.is_empty() {
        v["missing"] = json!(missing);
    }
    Ok(v)
}

fn calendar(args: &Value) -> Result<Value, String> {
    use chrono::NaiveDate;
    let day = |key: &str| -> Result<Option<NaiveDate>, String> {
        text(args, key)
            .map(|d| {
                NaiveDate::parse_from_str(d, "%Y-%m-%d")
                    .map_err(|_| format!("{key} is a day like 2026-11-02, not {d}"))
            })
            .transpose()
    };
    let from = day("from")?.unwrap_or_else(|| chrono::Local::now().date_naive());
    let to = day("to")?.unwrap_or(from + chrono::Duration::days(7));
    if to < from {
        return Err("to is before from".into());
    }
    let (from_key, to_key) = (
        from.format("%Y-%m-%d").to_string(),
        to.format("%Y-%m-%d").to_string(),
    );
    let mut items: Vec<Item> = super::load_all()
        .into_iter()
        .filter(|i| {
            !i.day().is_empty() && i.day() >= from_key.as_str() && i.day() <= to_key.as_str()
        })
        .collect();
    items.sort_by(|a, b| a.date.cmp(&b.date));
    let mut days: Vec<Value> = Vec::new();
    for item in &items {
        match days.last_mut() {
            Some(d) if d["day"] == item.day() => {
                if let Some(list) = d["items"].as_array_mut() {
                    list.push(brief(item));
                }
            }
            _ => days.push(json!({ "day": item.day(), "items": [brief(item)] })),
        }
    }
    Ok(json!({ "from": from_key, "to": to_key, "days": days }))
}

fn plan(args: &Value) -> Result<Value, String> {
    let title = required(args, "title")?;
    let date = required(args, "date")?;
    if chrono::NaiveDate::parse_from_str(date.get(..10).unwrap_or(date), "%Y-%m-%d").is_err() {
        return Err(format!(
            "date is a day like 2026-11-04, or 2026-11-04T09:00, not {date}"
        ));
    }
    let kind = kind_of(text(args, "kind").unwrap_or("video"))?;
    let item = super::plan(
        kind,
        title,
        date,
        text(args, "notes").unwrap_or(""),
        tags_of(args),
    )
    .map_err(io)?;
    Ok(brief(&item))
}

fn fulfil(args: &Value) -> Result<Value, String> {
    let item = super::fulfil(required(args, "plan")?, required(args, "id")?).map_err(io)?;
    Ok(brief(&item))
}

fn set_status(args: &Value) -> Result<Value, String> {
    let id = required(args, "id")?;
    let key = required(args, "status")?;
    let status = Status::from_key(key).ok_or(format!("no status {key}"))?;
    let item = super::get(id).ok_or(format!("no item {id}"))?;
    if matches!(item.kind, Kind::Blog | Kind::Social) && item.app == "publishkit" {
        return Err("a post's status is PublishKit's: publish or schedule it there".into());
    }
    super::set_status(id, status).map_err(io)?;
    get(&json!({ "id": id }))
}

fn set_tags(args: &Value) -> Result<Value, String> {
    let id = required(args, "id")?;
    super::set_tags(id, tags_of(args)).map_err(io)?;
    get(&json!({ "id": id }))
}

fn set_notes(args: &Value) -> Result<Value, String> {
    let id = required(args, "id")?;
    super::set_notes(id, args["notes"].as_str().unwrap_or("")).map_err(io)?;
    get(&json!({ "id": id }))
}

fn add(args: &Value) -> Result<Value, String> {
    let paths: Vec<PathBuf> = args["paths"]
        .as_array()
        .ok_or("paths is a list of absolute paths")?
        .iter()
        .filter_map(Value::as_str)
        .map(PathBuf::from)
        .collect();
    if let Some(p) = paths.iter().find(|p| !p.is_absolute() || !p.is_file()) {
        return Err(format!("{} isn't a file, as an absolute path", p.display()));
    }
    let added = super::add_files(&paths).map_err(io)?;
    Ok(json!({ "added": added.iter().map(brief).collect::<Vec<_>>() }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_is_answered_and_others_are_left_to_the_app() {
        for tool in tools() {
            let name = tool["name"].as_str().unwrap();
            assert!(
                call(name, &json!({ "id": "", "title": "" })).is_some(),
                "{name}"
            );
        }
        assert!(call("export", &json!({})).is_none());
        assert_eq!(kind_of("Video"), Ok(Kind::Video));
        assert!(plan(&json!({ "title": "x", "date": "soon" })).is_err());
    }
}
