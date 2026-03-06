use anyhow::{anyhow, Result};
use std::path::Path;

use crate::db;
use crate::editor;

pub struct Opts<'a> {
    pub status: Option<&'a str>,
    pub priority: Option<db::Priority>,
    pub effort: Option<db::Effort>,
    pub title: Option<&'a str>,
    pub desc: Option<&'a str>,
    pub json: bool,
}

pub fn run(root: &Path, id: &str, opts: Opts) -> Result<()> {
    let store = db::open(root)?;
    let task_id = db::resolve_task_id(&store, id, false)?;
    let ts = db::now_utc();

    let parsed_status = opts.status.map(db::parse_status).transpose()?;

    // If no fields were supplied, open the editor so the user can revise the
    // task's title and description interactively.
    let editor_title;
    let editor_desc;
    let (title_override, desc_override) = if opts.status.is_none()
        && opts.priority.is_none()
        && opts.effort.is_none()
        && opts.title.is_none()
        && opts.desc.is_none()
    {
        let interactive = std::env::var("TD_FORCE_EDITOR").is_ok()
            || std::io::IsTerminal::is_terminal(&std::io::stdin());
        if interactive {
            // Load the current task so we can pre-populate the template.
            let task = store
                .get_task(&task_id, false)?
                .ok_or_else(|| anyhow!("task not found"))?;

            let template = format!(
                "{}\n\
                 \n\
                 {}\n\
                 \n\
                 TD: Edit the title and description above. The first line is the\n\
                 TD: title; everything after the blank line is the description.\n\
                 TD: Lines starting with 'TD: ' will be ignored. Saving an empty\n\
                 TD: message aborts the update.",
                task.title, task.description,
            );

            let (t, d) = editor::open(&template)?;
            editor_title = t;
            editor_desc = d;
            (Some(editor_title.as_str()), Some(editor_desc.as_str()))
        } else {
            return Err(anyhow!(
                "nothing to update; provide at least one flag or run interactively to open an editor"
            ));
        }
    } else {
        (opts.title, opts.desc)
    };

    store.apply_and_persist(|doc| {
        let tasks = doc.get_map("tasks");
        let task = db::get_task_map(&tasks, &task_id)?.ok_or_else(|| anyhow!("task not found"))?;

        if let Some(s) = parsed_status {
            task.insert("status", db::status_label(s))?;
        }
        if let Some(p) = opts.priority {
            task.insert("priority", db::priority_label(p))?;
        }
        if let Some(e) = opts.effort {
            task.insert("effort", db::effort_label(e))?;
        }
        if let Some(t) = title_override {
            task.insert("title", t)?;
        }
        if let Some(d) = desc_override {
            task.insert("description", d)?;
        }
        task.insert("updated_at", ts.clone())?;
        Ok(())
    })?;

    if opts.json {
        let task = store
            .get_task(&task_id, false)?
            .ok_or_else(|| anyhow!("task not found"))?;
        println!("{}", serde_json::to_string(&task)?);
    } else {
        let c = crate::color::stdout_theme();
        println!("{}updated{} {}", c.green, c.reset, task_id);
    }

    Ok(())
}
