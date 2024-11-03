use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    http::{HeaderName, HeaderValue},
    response::IntoResponse,
    routing::get,
    Router,
};
use clap::Parser;
use foldhash::HashMap;
use lazy_comp::LazyComponents;
use notify_debouncer_full::{
    new_debouncer,
    notify::{EventKind, RecursiveMode},
    DebounceEventResult,
};
use std::{
    io::Write,
    net::SocketAddr,
    path::Path,
    sync::{Arc, LazyLock},
    time::Duration,
};
use tokio::sync::broadcast;
use tower_http::{
    compression::CompressionLayer, services::ServeDir, set_header::SetResponseHeaderLayer,
};

mod lazy_comp;

static ICONS: LazyLock<LazyComponents<'static, foldhash::fast::RandomState>> =
    LazyLock::new(|| lazy_comp::icons::<foldhash::fast::RandomState>());

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Port to run the server on
    #[arg(short, long, default_value_t = 3000)]
    port: u16,

    /// Directory for static files
    #[arg(long, name = "static", default_value = "static")]
    static_dir: String,

    /// Directory containing source HTML files
    #[arg(long, default_value = "site")]
    site: String,

    /// Directory for processed output
    #[arg(short = 'o', long, default_value = "build")]
    build: String,
}

#[tokio::main]
async fn main() {
    let args = Arc::new(Args::parse());

    // Create build directory if it doesn't exist
    fs_err::create_dir_all(&args.build).expect("Failed to create build directory");

    // Do initial build
    process_all_files(&args).expect("Failed to process files");

    // Channel for file change notifications
    let (tx, _) = broadcast::channel::<()>(16);
    let tx = Arc::new(tx);

    // Set up file watcher for HTML directory
    std::thread::spawn({
        let args = Arc::clone(&args);
        let tx = Arc::clone(&tx);

        move || {
            let mut watcher = new_debouncer(Duration::from_millis(80), None, {
                let args = Arc::clone(&args);
                move |res: DebounceEventResult| match res {
                    Ok(events) => {
                        // if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                        //     println!("updating files");
                        //     if let Err(e) = process_all_files(&args) {
                        //         eprintln!("Error processing files: {}", e);
                        //     }
                        //     tx.send(()).unwrap_or(0);
                        // }

                        if events
                            .iter()
                            .any(|e| matches!(e.kind, EventKind::Modify(_) | EventKind::Create(_)))
                        {
                            if let Err(e) = process_all_files(&args) {
                                eprintln!("Error processing files: {}", e);
                            }
                            tx.send(()).unwrap_or(0);
                        }
                    }
                    Err(e) => println!("Watch error: {:?}", e),
                }
            })
            .unwrap();

            // Watch both HTML and static directories
            watcher
                .watch(Path::new(&args.site), RecursiveMode::Recursive)
                .unwrap();

            fs_err::create_dir_all(&args.static_dir).unwrap();
            watcher
                .watch(Path::new(&args.static_dir), RecursiveMode::Recursive)
                .unwrap();

            std::thread::park();
        }
    });

    // Set up the router
    let app = Router::new()
        // Serve the build directory as the root
        .nest_service("/", ServeDir::new(&args.build))
        .layer(CompressionLayer::new().br(true).gzip(true))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("cache-control"),
            HeaderValue::from_static("no-store"),
        ))
        // WebSocket route for hot reload
        .route("/ws", get(ws_handler))
        .with_state(tx);

    // Start the server
    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));
    println!("Server running on http://{}", addr);
    println!("  Static files directory: {}", args.static_dir);
    println!("  HTML files directory: {}", args.site);
    println!("  Build directory: {}", args.build);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

// WebSocket handler for live reload
async fn ws_handler(
    ws: WebSocketUpgrade,
    axum::extract::State(tx): axum::extract::State<Arc<broadcast::Sender<()>>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws_client(socket, tx))
}

async fn handle_ws_client(mut socket: WebSocket, tx: Arc<broadcast::Sender<()>>) {
    let mut rx = tx.subscribe();

    while rx.recv().await.is_ok() {
        println!("sent reload!");
        if socket
            .send(Message::Text("reload".to_string()))
            .await
            .is_err()
        {
            break;
        }
    }
}

// Process all files in the HTML directory
fn process_all_files(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    // Clear build directory
    let _ = fs_err::remove_dir_all(&args.build);
    fs_err::create_dir_all(&args.build)?;

    // Copy static files to build directory
    copy_dir_all(&args.static_dir, &args.build)?;

    // Process HTML files
    process_site(&args.site, &args.build)?;

    // Inject hot reload script into all HTML files in build directory
    inject_hot_reload_into_build_dir(&args.build)?;
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
fn process_site(src_dir: &str, build_dir: &str) -> Result<(), Box<dyn std::error::Error>> {
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
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    let components = components
        .par_iter()
        .map(|c| wincomp::Component::new(c).map(|c| (c.root.name, c)))
        .collect::<Result<HashMap<_, _>, _>>()
        .unwrap();

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
    let _ = markdown_entries
        .into_iter()
        .map(|entry| {
            let path = entry.path();
            let markdown = fs_err::read_to_string(path)?;
            let document = markcomp::mdast::document(&mut markdown.as_str()).unwrap();

            let trimmed_entry = path.strip_prefix(src_dir).unwrap();
            let outpath = blog_build_dir.join(trimmed_entry);

            let base = outpath.parent().unwrap();
            let sans_extension = outpath.file_stem().unwrap();
            let outpath = base.join(sans_extension).join("index.html");
            paths.push(outpath.to_owned());

            if let Some(path) = outpath.parent() {
                fs_err::create_dir_all(path)?;
            }

            let mut output = Vec::new();
            write!(&mut output, "<Shell><article>");
            for node in document {
                node.write(&mut output).unwrap();
            }
            write!(&mut output, "</article></Shell>");
            fs_err::write(outpath, output)
        })
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    let _ = paths
        .par_iter()
        .map(|path| {
            let file = fs_err::read_to_string(path)?;

            let mut document = match wincomp::Document::new(&file) {
                Ok(d) => d,
                Err(e) => panic!("{e}"),
            };
            document.expand(|name| components.get(name), |name| (&*ICONS).get(name));

            let trimmed_entry = if path.starts_with(src_dir) {
                path.strip_prefix(src_dir).unwrap()
            } else {
                path.strip_prefix(&blog_build_dir).unwrap()
            };
            let outpath = build_dir.join(trimmed_entry);

            if let Some(path) = outpath.parent() {
                fs_err::create_dir_all(path)?;
            }

            let mut buffer = Vec::new();
            document.write(&mut buffer)?;
            fs_err::write(outpath, buffer)
        })
        .collect::<Result<Vec<_>, _>>()?;

    fs_err::write(build_dir.join("output.css"), combined_css)?;

    let elapsed = std::time::Instant::now() - start;

    println!(
        "Processed {} files in {}ms",
        components.len() + paths.len(),
        elapsed.as_millis()
    );

    Ok(())
}

// Inject hot reload script into all HTML files in the build directory
fn inject_hot_reload_into_build_dir(build_dir: &str) -> Result<(), Box<dyn std::error::Error>> {
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

fn inject_css_into_build_dir(build_dir: &str) -> Result<(), Box<dyn std::error::Error>> {
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
