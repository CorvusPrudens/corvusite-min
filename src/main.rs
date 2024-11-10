use anyhow::Context;
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    http::{HeaderName, HeaderValue},
    response::IntoResponse,
    routing::get,
    Router,
};
use clap::{Args as ClapArgs, Parser, Subcommand};
use notify_debouncer_full::{
    new_debouncer,
    notify::{EventKind, RecursiveMode},
    DebounceEventResult,
};
use std::{net::SocketAddr, path::Path, sync::Arc, time::Duration};
use tokio::sync::broadcast;
use tower_http::{
    compression::CompressionLayer, services::ServeDir, set_header::SetResponseHeaderLayer,
};

mod gen;
mod lazy_comp;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,

    #[command(flatten)]
    options: Options,
}

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct Options {
    /// Directory for processed output
    #[arg(short = 'o', long, default_value = "build", global = true)]
    build: String,

    /// Directory for static files
    #[arg(long, name = "static", default_value = "static", global = true)]
    static_dir: String,

    /// Directory containing source HTML files
    #[arg(long, default_value = "site", global = true)]
    site: String,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    Build,
    Serve(ServeArgs),
}

#[derive(ClapArgs, Debug, Clone)]
struct ServeArgs {
    /// Port to run the server on
    #[arg(short, long, default_value_t = 3000)]
    port: u16,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Create build directory if it doesn't exist
    fs_err::create_dir_all(&args.options.build).context("Failed to create build directory")?;

    match args.command {
        Commands::Build => {
            if let Err(e) = gen::process_all_files(&args.options, false) {
                eprintln!("Error processing files: {e}");
            }
        }
        Commands::Serve(serve_args) => {
            // Start the Tokio runtime
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                if let Err(e) = serve(args.options, serve_args).await {
                    eprintln!("Server error: {e}");
                }
            });
        }
    }

    Ok(())
}

async fn serve(options: Options, serve_args: ServeArgs) -> Result<(), Box<dyn std::error::Error>> {
    let context = Arc::new(options);

    let site_dir = &context.site;
    let static_dir = &context.static_dir;
    let port = serve_args.port;

    // Create build directory if it doesn't exist
    fs_err::create_dir_all(&context.build).expect("Failed to create build directory");

    // Do initial build
    if let Err(e) = gen::process_all_files(&context, true) {
        eprintln!("Error processing files: {e}");
    }

    // Channel for file change notifications
    let (tx, _) = broadcast::channel::<()>(16);
    let tx = Arc::new(tx);

    // Set up file watcher for HTML directory
    std::thread::spawn({
        let context = Arc::clone(&context);
        let tx = Arc::clone(&tx);

        move || {
            let mut watcher = new_debouncer(Duration::from_millis(150), None, {
                let context = Arc::clone(&context);
                move |res: DebounceEventResult| match res {
                    Ok(events) => {
                        if events
                            .iter()
                            .any(|e| matches!(e.kind, EventKind::Modify(_) | EventKind::Create(_)))
                        {
                            if let Err(e) = gen::process_all_files(&context, true) {
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
                .watch(Path::new(&context.site), RecursiveMode::Recursive)
                .unwrap();

            fs_err::create_dir_all(&context.static_dir).unwrap();
            watcher
                .watch(Path::new(&context.static_dir), RecursiveMode::Recursive)
                .unwrap();

            std::thread::park();
        }
    });

    // Set up the router
    let app = Router::new()
        // Serve the build directory as the root
        .nest_service("/", ServeDir::new(&context.build))
        .layer(CompressionLayer::new().br(true).gzip(true))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("cache-control"),
            HeaderValue::from_static("no-store"),
        ))
        // WebSocket route for hot reload
        .route("/ws", get(ws_handler))
        .with_state(tx);

    // Start the server
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("Server running on http://{}", addr);
    println!("  Static files directory: {}", static_dir);
    println!("  HTML files directory: {}", site_dir);
    println!("  Build directory: {}", context.build);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();

    Ok(())
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
