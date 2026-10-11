use axum::Router;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use maud::{DOCTYPE, Markup, PreEscaped, html};
use tokio::sync::watch;

use crate::api::upstream_failure_status;
use crate::park_status::Verdict;
use crate::poller::Latest;

const QUESTION: &str = "¿Está abierto el Retiro?";

const STYLE: &str = include_str!("page.css");

const SOURCE_URL: &str = "https://www.madrid.es/portal/site/munimadrid/menuitem.0c57021e0d1f6162c345c34571f1a5a0/?vgnextfmt=default&vgnextoid=51c3fb4f3f2cc710VgnVCM1000001d4a900aRCRD&vgnextchannel=3edd31d3b28fe410VgnVCM1000000b205a0aRCRD&vgnextlocale=es";
const LICENCE_URL: &str = "https://creativecommons.org/licenses/by/4.0/deed.es";

pub fn router(latest: watch::Receiver<Latest>) -> Router {
    Router::new()
        .route("/", get(retiro_status))
        .with_state(latest)
}

async fn retiro_status(State(latest): State<watch::Receiver<Latest>>) -> Response {
    match &*latest.borrow() {
        Ok(fetched) => render(headline(fetched.status.code.verdict())).into_response(),
        Err(error) => {
            let (status, _) = upstream_failure_status(error);
            (status, render("No hemos podido consultar el estado")).into_response()
        }
    }
}

fn render(answer: &str) -> Markup {
    html! {
        (DOCTYPE)
        html lang="es" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (QUESTION) }
                style { (PreEscaped(STYLE)) }
            }
            body {
                p { (QUESTION) }
                h1 { (answer) }
                footer {
                    "Elaborado a partir de "
                    a href=(SOURCE_URL) { "datos del Ayuntamiento de Madrid" }
                    " ("
                    a href=(LICENCE_URL) { "CC BY 4.0" }
                    "). Esta web interpreta esos datos y no está vinculada al Ayuntamiento ni avalada por él."
                }
            }
        }
    }
}

fn headline(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Open => "Abierto",
        Verdict::OpenWithRestrictions => "Abierto con restricciones",
        Verdict::OpenStayNotAdvised => "Abierto, pero no se recomienda permanecer",
        Verdict::ClosureForecast => "Cierre previsto",
        Verdict::Closed => "Cerrado",
        Verdict::Unknown => "Estado desconocido",
    }
}
