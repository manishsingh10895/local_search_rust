use std::{
    fs::File,
    sync::{Arc, Mutex},
};

use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use crate::model::Model;

fn serve_404(request: Request) -> Result<(), ()> {
    request
        .respond(Response::from_string("404").with_status_code(StatusCode(404)))
        .map_err(|err| {
            eprintln!("Something is not found :{err}");
        })
}

fn serve_500(request: Request) -> Result<(), ()> {
    request
        .respond(Response::from_string("500").with_status_code(StatusCode(500)))
        .map_err(|err| {
            eprintln!("Something is not right :{err}");
        })
}

fn serve_api_search(model: Arc<Mutex<Model>>, mut request: tiny_http::Request) -> Result<(), ()> {
    let mut buf = Vec::<u8>::new();
    request.as_reader().read_to_end(&mut buf).map_err(|err| {
        eprintln!("ERROR: Cannot read request body : {err}");
    })?;

    let body = std::str::from_utf8(&buf)
        .map_err(|err| {
            eprintln!("ERROR: Cannot interpret body at UTF-8 string: {err}");
        })?
        .chars()
        .collect::<Vec<_>>();

    let model = model.lock().unwrap();

    let results = model.search_query(&body)?;

    let json = match serde_json::to_string(&results.iter().take(20).collect::<Vec<_>>()) {
        Ok(json) => json,
        Err(err) => {
            eprintln!("ERROR: could not convert search results to JSON: {err}");
            return serve_500(request);
        }
    };

    let content_type_header =
        Header::from_bytes("Content-Type", "application/json").expect("No garbage in header");

    let _x = request
        .respond(Response::from_string(&json).with_header(content_type_header))
        .unwrap();

    Ok(())
}

fn serve_static_file(request: Request, file_path: &str, content_type: &str) -> Result<(), ()> {
    let content_type_header =
        Header::from_bytes("Content-Type", content_type).expect("No invalid header");

    let file = File::open(file_path).map_err(|err| {
        eprintln!("ERROR: could not serve file {file_path}: {err}");
    })?;

    let response = Response::from_file(file).with_header(content_type_header);

    request.respond(response).map_err(|err| {
        eprintln!("ERROR: could not serve static file {file_path}: {err}");
    })
}

fn serve_request(model: Arc<Mutex<Model>>, request: tiny_http::Request) -> Result<(), ()> {
    println!(
        "INFO: Received request method: {:?}, url: {:?}",
        request.method(),
        request.url()
    );

    match (request.method(), request.url()) {
        (Method::Post, "/api/search") => serve_api_search(model, request),
        (Method::Get, "/index.js") => {
            serve_static_file(request, "index.js", "text/javascript; charset=utf-8")
        }
        (Method::Get, "/") | (Method::Get, "index.html") => {
            serve_static_file(request, "index.html", "text/html;")
        }
        _ => serve_404(request),
    }
}

pub fn start(address: &str, model: Arc<Mutex<Model>>) -> Result<(), ()> {
    let server = Server::http(&address).map_err(|err| {
        eprintln!("ERROR: couldnot start the server at {address}: {err}");
    })?;

    println!("INFO: Listening at HTTP server at {address}");

    for request in server.incoming_requests() {
        // convert to option, to not break on errors
        serve_request(Arc::clone(&model), request)
            .map_err(|err| {
                eprintln!("ERROR: couldnot serve reponse: {err:?}");
            })
            .ok();
    }

    eprintln!("ERROR: the server socket has shutdown");

    Err(())
}
