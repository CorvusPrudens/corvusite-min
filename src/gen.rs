use crate::lazy_comp::{icons, LazyComponents};
use crate::Options;
use anyhow::{anyhow, bail, Error};
use foldhash::HashMap;
use std::io::Write;
use std::path::Path;
use std::sync::LazyLock;

pub static ICONS: LazyLock<LazyComponents<'static, foldhash::fast::RandomState>> =
    LazyLock::new(icons::<foldhash::fast::RandomState>);

// Process all files in the HTML directory
pub(crate) fn process_all_files(args: &Options, inject_reload: bool) -> Result<(), Error> {
    // Clear build directory
    let _ = fs_err::remove_dir_all(&args.build);
    fs_err::create_dir_all(&args.build)?;

    // Copy static files to build directory
    copy_dir_all(&args.static_dir, &args.build)?;

    // Process HTML files
    process_site(&args.site, &args.build)?;

    // Inject hot reload script into all HTML files in build directory
    if inject_reload {
        inject_hot_reload_into_build_dir(&args.build)?;
    }
    inject_css_into_build_dir(&args.build)?;

    Ok(())
}

// Helper function to recursively copy directories
fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    fs_err::create_dir_all(&dst)?;

    let Ok(entries) = fs_err::read_dir(src.as_ref()) else {
        return Ok(());
    };

    for entry in entries {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs_err::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

// Process HTML files (placeholder - implement your preprocessor here)
fn process_site(src_dir: &str, build_dir: &str) -> Result<(), Error> {
    let src_dir = Path::new(src_dir);
    let build_dir = Path::new(build_dir);
    let mut combined_css = Vec::new();

    let start = std::time::Instant::now();

    // pass one
    let mut component_entries = Vec::new();
    let mut markdown_entries = Vec::new();
    for entry in walkdir::WalkDir::new(src_dir)
        .into_iter()
        .filter_map(|f| match f {
            Ok(f) => (!f.path().is_dir()).then_some(f),
            _ => None,
        })
    {
        let path = entry.path();
        let path_string = path.to_string_lossy();

        if path_string.ends_with(".mod.html") {
            component_entries.push(entry);
        } else if path_string.ends_with(".css") {
            combined_css.extend(fs_err::read(path)?);
        } else if path_string.ends_with(".md") {
            markdown_entries.push(entry);
        }
    }

    use rayon::prelude::*;

    let components = component_entries
        .into_par_iter()
        .map(|entry| fs_err::read_to_string(entry.path()))
        .collect::<Result<Vec<_>, _>>()?;

    let result = components
        .par_iter()
        .map(|c| wincomp::Component::new(c).map(|c| (c.root.name, c)))
        .collect::<Result<HashMap<_, _>, _>>();

    let components = match result {
        Ok(c) => c,
        Err(e) => bail!("Error processing components: {e}"),
    };

    let mut paths: Vec<_> = walkdir::WalkDir::new(src_dir)
        .into_iter()
        .filter_map(|f| match f {
            Ok(f) => {
                if f.path().is_dir() {
                    None
                } else {
                    let string = f.path().to_string_lossy();
                    if !string.ends_with(".mod.html") && string.ends_with(".html") {
                        Some(f.path().to_owned())
                    } else {
                        None
                    }
                }
            }
            _ => None,
        })
        .collect();

    let blog_build_dir = build_dir.join("blog-build");
    let mut articles = Vec::new();
    markdown_entries
        .into_iter()
        .map(|entry| {
            let path = entry.path();

            let trimmed_entry = path.strip_prefix(src_dir)?;
            let outpath = blog_build_dir.join(trimmed_entry);

            let base = outpath
                .parent()
                .ok_or(anyhow!("Blog file has no parent path"))?;
            let sans_extension = outpath
                .file_stem()
                .ok_or(anyhow!("Blog file has no file stem"))?;
            let outpath = base.join(sans_extension).join("index.html");
            paths.push(outpath.to_owned());

            if let Some(path) = outpath.parent() {
                fs_err::create_dir_all(path)?;
            }

            let markdown = fs_err::read_to_string(path)?;
            let mut output = Vec::new();
            let mut markdown = markcomp::pull::Writer::new(&markdown)?;

            let frontmatter = markdown
                .frontmatter
                .take()
                .ok_or(anyhow!("Missing frontmatter in {path:?}"))?;

            let date = jiff::fmt::strtime::parse("%D", &frontmatter.date)?.to_date()?;

            write!(
                &mut output,
                r#"<html lang="en"><ShellHead><title>{} | Corvus Prudens</title></ShellHead><ShellBody><article>"#,
                frontmatter.title
            )?;

            articles.push((
                date,
                sans_extension.to_string_lossy().to_string(),
                frontmatter,
            ));
            let mut markdown = markdown.output();

            output.append(&mut markdown);
            write!(&mut output, "</article></ShellBody></html>")?;
            fs_err::write(outpath, output)?;

            Ok(())
        })
        .collect::<Result<Vec<_>, Error>>()?;

    // Create blog index
    articles.sort_by_key(|s| std::cmp::Reverse(s.0));
    let path = blog_build_dir.join("blog").join("index.html");
    let data = format!(
        "<BlogShell>{}</BlogShell>",
        articles
            .into_iter()
            .map(|(date, path, frontmatter)| {
                format!(
                    r#"
                        <BlogCard>
                            <div class="title-items">
                                <BlogLink href="/blog/{path}/">
                                    {}
                                </BlogLink>
                                <BlogDate>
                                    {}
                                </BlogDate>
                            </div>
                            <BlogDescription>
                                {}
                            </BlogDescription>
                        </BlogCard>"#,
                    frontmatter.title,
                    jiff::fmt::strtime::format("%D", date).unwrap(),
                    frontmatter.description,
                )
            })
            .collect::<Vec<_>>()
            .join("")
    );
    fs_err::write(&path, data.as_bytes())?;
    paths.push(path);

    paths
        .par_iter()
        .map(|path| {
            let file = fs_err::read_to_string(path)?;

            let mut document = match wincomp::Document::new(&file) {
                Ok(d) => d,
                Err(e) => bail!("Error processing {path:?}: {e}"),
            };
            document.expand(|name| components.get(name).or_else(|| ICONS.get(name)));

            let trimmed_entry = if path.starts_with(src_dir) {
                path.strip_prefix(src_dir)
            } else {
                path.strip_prefix(&blog_build_dir)
            }
            .map_err(|e| anyhow!("No prefix on target file: {e}"))?;

            let outpath = build_dir.join(trimmed_entry);

            if let Some(path) = outpath.parent() {
                fs_err::create_dir_all(path)?;
            }

            let mut buffer = Vec::new();
            document.write(&mut buffer)?;
            fs_err::write(outpath, buffer)?;

            Ok(())
        })
        .collect::<Result<Vec<_>, Error>>()?;

    fs_err::write(build_dir.join("output.css"), combined_css)?;
    // fs_err::remove_dir_all(blog_build_dir)?;

    let elapsed = std::time::Instant::now() - start;

    println!(
        "Processed {} files in {}us",
        components.len() + paths.len(),
        elapsed.as_micros()
    );

    Ok(())
}

fn inject_hot_reload_into_build_dir(build_dir: &str) -> Result<(), Error> {
    let script = r#"
        <script>
            const ws = new WebSocket(`ws://${location.host}/ws`);
            ws.onmessage = () => location.reload();
        </script>
    "#;

    fn inject_into_dir(dir: &Path, script: &str) -> std::io::Result<()> {
        for entry in fs_err::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                inject_into_dir(&path, script)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("html") {
                let content = fs_err::read_to_string(&path)?;
                let modified = content.replace("</body>", &format!("{script}</body>"));
                fs_err::write(path, modified)?;
            }
        }
        Ok(())
    }

    inject_into_dir(Path::new(build_dir), script)?;
    Ok(())
}

fn inject_css_into_build_dir(build_dir: &str) -> Result<(), Error> {
    let css = r#"
        <link rel="stylesheet" type="text/css" href="/output.css">
    "#;

    fn inject_into_dir(dir: &Path, script: &str) -> std::io::Result<()> {
        for entry in fs_err::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                inject_into_dir(&path, script)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("html") {
                let content = fs_err::read_to_string(&path)?;
                let modified = content.replace("</head>", &format!("{script}</head>"));
                fs_err::write(path, modified)?;
            }
        }
        Ok(())
    }

    inject_into_dir(Path::new(build_dir), css)?;
    Ok(())
}
