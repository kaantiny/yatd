use anyhow::Result;

pub fn run(json: bool) -> Result<()> {
    let projects = crate::db::list_projects()?;

    if json {
        println!("{}", serde_json::to_string(&projects)?);
    } else {
        for project in projects {
            println!("{project}");
        }
    }

    Ok(())
}
