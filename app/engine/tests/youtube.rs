//! Transformation `youtube`, contre un faux service oEmbed local : aucun test ne
//! touche Internet.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use engine::{Config, Endpoints, Engine, Outcome, SAMPLE_CONFIG};
use tokio::sync::mpsc;

const VIDEO: &str = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";

/// Réponse du faux service pour `VIDEO` (titre avec accents et accolades).
const VIDEO_JSON: &str = r#"{
  "title": "Rick Astley — Never Gonna {author_name} Give You Up",
  "author_name": "Rick Astley",
  "author_url": "https://www.youtube.com/@RickAstleyYT",
  "type": "video",
  "thumbnail_url": "https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg",
  "provider_name": "YouTube"
}"#;

/// Comportement du faux service oEmbed.
#[derive(Clone)]
enum Fake {
    /// Connaît `VIDEO` (200) ; toute autre URL : 404.
    Knows,
    /// Ne répond qu'après ce délai.
    Slow(Duration),
    /// Rend toujours ce statut (« 200 OK ») et ce corps.
    Answers(&'static str, String),
    /// Annonce un corps plus long que celui envoyé, puis coupe la connexion.
    Cuts,
    /// Redirige (302) `/oembed?…` vers `/moved?…`, qui se comporte comme `Knows`.
    Redirects,
    /// Ne répond jamais ; signale `Heard` à la requête reçue, puis `HungUp`
    /// quand le client ferme la connexion.
    Hangs(mpsc::UnboundedSender<Hang>),
}

#[derive(Debug, PartialEq, Eq)]
enum Hang {
    Heard,
    HungUp,
}

/// Lance un faux service oEmbed sur un port local libre ; rend son adresse.
fn fake_oembed(fake: Fake) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/oembed", listener.local_addr().unwrap());
    thread::spawn(move || {
        for stream in listener.incoming() {
            let stream = stream.unwrap();
            let fake = fake.clone();
            thread::spawn(move || serve(stream, fake));
        }
    });
    address
}

fn serve(mut stream: TcpStream, fake: Fake) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    reader.read_line(&mut request_line).unwrap();
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).unwrap() == 0 || header == "\r\n" {
            break;
        }
    }
    // `GET /oembed?url=…&format=json HTTP/1.1` : l'URL copiée doit arriver encodée.
    let path = request_line.split(' ').nth(1).unwrap_or_default();
    let expected =
        "/oembed?url=https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3DdQw4w9WgXcQ&format=json";
    let known = || {
        if path == expected {
            ("200 OK", VIDEO_JSON.to_owned())
        } else {
            ("404 Not Found", "Not Found".to_owned())
        }
    };
    if let Fake::Hangs(events) = &fake {
        let _ = events.send(Hang::Heard);
        // Bloque jusqu'à ce que le client coupe : lecture à 0 octet, ou erreur.
        let _ = reader.read(&mut [0; 1]);
        let _ = events.send(Hang::HungUp);
        return;
    }
    let mut location = String::new();
    let (status, body, length) = match fake {
        Fake::Knows => {
            let (status, body) = known();
            (status, body.clone(), body.len())
        }
        Fake::Slow(delay) => {
            thread::sleep(delay);
            let (status, body) = known();
            (status, body.clone(), body.len())
        }
        Fake::Hangs(_) => unreachable!(),
        Fake::Answers(status, body) => (status, body.clone(), body.len()),
        Fake::Cuts => ("200 OK", r#"{"title": "Ri"#.to_owned(), 500),
        Fake::Redirects => match path.strip_prefix("/moved") {
            Some(query) if format!("/oembed{query}") == expected => {
                ("200 OK", VIDEO_JSON.to_owned(), VIDEO_JSON.len())
            }
            Some(_) => ("404 Not Found", String::new(), 0),
            None => {
                let query = path.strip_prefix("/oembed").unwrap_or_default();
                location = format!("Location: /moved{query}\r\n");
                ("302 Found", String::new(), 0)
            }
        },
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {length}\r\n{location}Connection: close\r\n\r\n{body}"
    );
}

/// Moteur dont l'unique Règle applique l'étape `step` (YAML) à toute copie.
fn engine_with(step: &str, oembed: String) -> Engine {
    let yaml = format!(
        "config_version: 1\npipelines:\n  p:\n    - {step}\nrules:\n  - match: ''\n    pipeline: p\n"
    );
    Engine::new(Config::parse_with(&yaml, &Endpoints { oembed }).expect("config valide"))
}

#[tokio::test]
async fn default_template_is_a_markdown_link_to_the_copied_url() {
    let engine = engine_with("youtube", fake_oembed(Fake::Knows));
    assert_eq!(
        engine.on_copy(VIDEO).await,
        Outcome::Write(format!(
            "[Rick Astley — Never Gonna {{author_name}} Give You Up]({VIDEO})"
        ))
    );
}

#[tokio::test]
async fn a_custom_template_can_use_every_field_and_the_copied_url_is_trimmed() {
    let engine = engine_with(
        r#"youtube: {template: "{author_name} | {title} | {url} | {author_url} | {thumbnail_url} | {not a field}"}"#,
        fake_oembed(Fake::Knows),
    );
    assert_eq!(
        engine.on_copy(&format!("  {VIDEO}\n")).await,
        Outcome::Write(format!(
            "Rick Astley | Rick Astley — Never Gonna {{author_name}} Give You Up | {VIDEO} \
             | https://www.youtube.com/@RickAstleyYT \
             | https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg | {{not a field}}"
        ))
    );
}

#[tokio::test]
async fn the_sample_config_turns_a_video_url_into_a_link_with_its_channel() {
    let oembed = fake_oembed(Fake::Knows);
    let engine = Engine::new(Config::parse_with(SAMPLE_CONFIG, &Endpoints { oembed }).unwrap());
    assert_eq!(
        engine.on_copy(VIDEO).await,
        Outcome::Write(format!(
            "[Rick Astley — Never Gonna {{author_name}} Give You Up]({VIDEO}) \
             — [Rick Astley](https://www.youtube.com/@RickAstleyYT)"
        ))
    );
}

#[tokio::test]
async fn an_unknown_video_fails_the_pipeline() {
    let engine = engine_with("youtube", fake_oembed(Fake::Knows));
    let Outcome::Failed(failure) = engine
        .on_copy("https://www.youtube.com/watch?v=doesNotExist")
        .await
    else {
        panic!("attendu un échec");
    };
    assert_eq!(failure.step, "youtube");
    assert!(
        failure.message.contains("introuvable"),
        "{}",
        failure.message
    );
    assert!(failure.message.contains("404"), "{}", failure.message);
}

#[tokio::test]
async fn a_response_slower_than_the_timeout_fails_the_pipeline_on_time() {
    let engine = engine_with(
        "{youtube: null, timeout: 0.3}",
        fake_oembed(Fake::Slow(Duration::from_secs(5))),
    );
    let start = Instant::now();
    let outcome = engine.on_copy(VIDEO).await;
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "{:?}",
        start.elapsed()
    );
    let Outcome::Failed(failure) = outcome else {
        panic!("attendu un échec, obtenu {outcome:?}");
    };
    assert_eq!(failure.step, "youtube");
    assert!(failure.message.contains("0.3 s"), "{}", failure.message);
}

#[tokio::test]
async fn a_newer_copy_drops_the_pending_request() {
    let (events, mut heard) = mpsc::unbounded_channel();
    let yaml = "config_version: 1\npipelines:\n  link: [youtube]\n  shout: [upper]\nrules:\n  - match: youtube\n    pipeline: link\n  - match: ''\n    pipeline: shout\n";
    let oembed = fake_oembed(Fake::Hangs(events));
    let engine = Engine::new(Config::parse_with(yaml, &Endpoints { oembed }).unwrap());
    let soon = Duration::from_secs(2);
    let started = Instant::now();
    let video = tokio::spawn(engine.on_copy(VIDEO));
    // La requête est partie avant qu'on copie autre chose.
    let first = tokio::time::timeout(soon, heard.recv()).await;
    assert_eq!(first, Ok(Some(Hang::Heard)));

    assert_eq!(engine.on_copy("abc").await, Outcome::Write("ABC".into()));
    assert_eq!(video.await.unwrap(), Outcome::Cancelled);
    assert!(started.elapsed() < soon, "{:?}", started.elapsed());
    let second = tokio::time::timeout(soon, heard.recv()).await;
    assert_eq!(
        second,
        Ok(Some(Hang::HungUp)),
        "la requête n'a pas été coupée"
    );
}

/// Message d'échec de `youtube` sur une copie de `VIDEO` quand oEmbed répond ainsi.
async fn failure_when_oembed_answers(status: &'static str, body: &str) -> String {
    let engine = engine_with("youtube", fake_oembed(Fake::Answers(status, body.into())));
    match engine.on_copy(VIDEO).await {
        Outcome::Failed(failure) => {
            assert_eq!(failure.step, "youtube");
            failure.message
        }
        other => panic!("attendu un échec, obtenu {other:?}"),
    }
}

#[tokio::test]
async fn brackets_and_backslashes_in_title_and_channel_do_not_break_the_link() {
    let body = r#"{"title": "Part 1] (live) | [HD] \\ x", "author_name": "A]B", "author_url": "https://c"}"#;
    let engine = engine_with(
        r#"youtube: {template: "[{title}]({url}) — [{author_name}]({author_url})"}"#,
        fake_oembed(Fake::Answers("200 OK", body.into())),
    );
    assert_eq!(
        engine.on_copy(VIDEO).await,
        Outcome::Write(format!(
            r"[Part 1\] (live) | \[HD\] \\ x]({VIDEO}) — [A\]B](https://c)"
        ))
    );
}

#[tokio::test]
async fn an_answer_without_title_fails_the_pipeline() {
    for body in ["{}", r#"{"title": ""}"#, r#"{"error": "Unauthorized"}"#] {
        let message = failure_when_oembed_answers("200 OK", body).await;
        assert!(message.contains("sans titre"), "{body} : {message}");
    }
}

#[tokio::test]
async fn an_answer_that_is_not_json_fails_the_pipeline() {
    let message = failure_when_oembed_answers("200 OK", "<html>Hello</html>").await;
    assert!(message.contains("illisible"), "{message}");
}

#[tokio::test]
async fn only_http_200_is_a_success() {
    let message = failure_when_oembed_answers("201 Created", VIDEO_JSON).await;
    assert!(message.contains("201"), "{message}");
}

#[tokio::test]
async fn a_refused_video_fails_the_pipeline_and_blames_the_video() {
    for status in ["400 Bad Request", "401 Unauthorized", "403 Forbidden"] {
        let message = failure_when_oembed_answers(status, "").await;
        assert!(message.contains(&status[..3]), "{message}");
        assert!(message.contains("vidéo privée"), "{message}");
    }
}

#[tokio::test]
async fn a_redirect_is_followed() {
    let engine = engine_with("youtube", fake_oembed(Fake::Redirects));
    assert_eq!(
        engine.on_copy(VIDEO).await,
        Outcome::Write(format!(
            "[Rick Astley — Never Gonna {{author_name}} Give You Up]({VIDEO})"
        ))
    );
}

#[tokio::test]
async fn urls_are_never_escaped() {
    let body = r#"{"title": "T", "author_name": "A", "author_url": "https://c/[a]", "thumbnail_url": "https://t/[b]"}"#;
    let engine = engine_with(
        r#"youtube: {template: "{url} {author_url} {thumbnail_url}"}"#,
        fake_oembed(Fake::Answers("200 OK", body.into())),
    );
    assert_eq!(
        engine.on_copy("https://youtu.be/x?list=[1]").await,
        Outcome::Write("https://youtu.be/x?list=[1] https://c/[a] https://t/[b]".into())
    );
}

#[tokio::test]
async fn rate_limiting_and_server_errors_do_not_blame_the_video() {
    let limited = failure_when_oembed_answers("429 Too Many Requests", "").await;
    assert!(limited.contains("429"), "{limited}");
    assert!(limited.contains("limite"), "{limited}");
    let broken = failure_when_oembed_answers("500 Internal Server Error", "").await;
    assert!(broken.contains("500"), "{broken}");
    assert!(broken.contains("erreur du service"), "{broken}");
    for message in [limited, broken] {
        assert!(!message.contains("vidéo"), "{message}");
    }
}

#[tokio::test]
async fn an_oversized_answer_fails_the_pipeline() {
    let body = format!(r#"{{"title": "{}"}}"#, "x".repeat(70 * 1024));
    let message = failure_when_oembed_answers("200 OK", &body).await;
    assert!(message.contains("trop grosse"), "{message}");
}

#[tokio::test]
async fn an_answer_cut_short_fails_the_pipeline() {
    let engine = engine_with("youtube", fake_oembed(Fake::Cuts));
    let Outcome::Failed(failure) = engine.on_copy(VIDEO).await else {
        panic!("attendu un échec");
    };
    assert!(
        failure.message.contains("interrompue"),
        "{}",
        failure.message
    );
    // Le message part dans le journal du service : jamais le texte copié.
    assert!(
        !failure.message.contains("dQw4w9WgXcQ"),
        "{}",
        failure.message
    );
}

/// Horloge de test figée : elle avance d'un coup jusqu'au délai, sans attendre 10 s.
#[tokio::test(start_paused = true)]
async fn without_timeout_key_the_step_gives_up_after_ten_seconds() {
    // Le faux service ne répond qu'après une heure (en temps réel).
    let engine = engine_with(
        "youtube",
        fake_oembed(Fake::Slow(Duration::from_secs(3600))),
    );
    let Outcome::Failed(failure) = engine.on_copy(VIDEO).await else {
        panic!("attendu un échec");
    };
    assert_eq!(failure.message, "pas de réponse d'oEmbed en 10 s");
}

#[tokio::test]
async fn an_unreachable_service_fails_the_pipeline() {
    // Port local libéré aussitôt réservé : personne n'y écoute.
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let engine = engine_with("youtube", format!("http://127.0.0.1:{port}/oembed"));
    let Outcome::Failed(failure) = engine.on_copy(VIDEO).await else {
        panic!("attendu un échec");
    };
    assert!(
        failure.message.contains("injoignable"),
        "{}",
        failure.message
    );
    // Le message part dans le journal du service : jamais le texte copié.
    assert!(
        !failure.message.contains("dQw4w9WgXcQ"),
        "{}",
        failure.message
    );
}

#[test]
fn an_unknown_template_field_is_refused_when_loading_the_config() {
    let yaml =
        "config_version: 1\npipelines:\n  p:\n    - youtube: {template: '[{titel}]({url})'}\n";
    let err = Config::parse(yaml).err().expect("config refusée");
    assert_eq!(err.field, "pipelines.p[0].youtube.template");
    assert!(err.reason.contains("{titel}"), "{}", err.reason);
}
