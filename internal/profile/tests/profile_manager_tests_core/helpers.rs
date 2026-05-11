use super::*;

pub(super) fn seed_chromium_cookies(path: &std::path::Path) {
    let conn = Connection::open(path).expect("open cookies");
    conn.execute_batch(
        "CREATE TABLE cookies (
            host_key TEXT NOT NULL,
            name TEXT NOT NULL,
            value TEXT NOT NULL
        );",
    )
    .expect("schema");
    conn.execute(
        "INSERT INTO cookies(host_key, name, value) VALUES (?1, ?2, ?3)",
        params![".keep.example.com", "sid", "1"],
    )
    .expect("keep cookie");
    conn.execute(
        "INSERT INTO cookies(host_key, name, value) VALUES (?1, ?2, ?3)",
        params![".drop.example.com", "sid", "2"],
    )
    .expect("drop cookie");
}

pub(super) fn seed_chromium_history(path: &std::path::Path) {
    let conn = Connection::open(path).expect("open history");
    conn.execute_batch(
        "CREATE TABLE urls (
            id INTEGER PRIMARY KEY,
            url TEXT NOT NULL
        );
        CREATE TABLE visits (
            id INTEGER PRIMARY KEY,
            url INTEGER NOT NULL
        );",
    )
    .expect("schema");
    conn.execute(
        "INSERT INTO urls(id, url) VALUES (1, ?1)",
        params!["https://keep.example.com/path"],
    )
    .expect("keep url");
    conn.execute(
        "INSERT INTO urls(id, url) VALUES (2, ?1)",
        params!["https://drop.example.com/path"],
    )
    .expect("drop url");
    conn.execute("INSERT INTO visits(id, url) VALUES (1, 1)", [])
        .expect("keep visit");
    conn.execute("INSERT INTO visits(id, url) VALUES (2, 2)", [])
        .expect("drop visit");
}

pub(super) fn seed_firefox_cookies(path: &std::path::Path) {
    let conn = Connection::open(path).expect("open cookies");
    conn.execute_batch(
        "CREATE TABLE moz_cookies (
            id INTEGER PRIMARY KEY,
            host TEXT NOT NULL
        );",
    )
    .expect("schema");
    conn.execute(
        "INSERT INTO moz_cookies(id, host) VALUES (1, ?1)",
        params![".keep.example.com"],
    )
    .expect("keep cookie");
    conn.execute(
        "INSERT INTO moz_cookies(id, host) VALUES (2, ?1)",
        params![".drop.example.com"],
    )
    .expect("drop cookie");
}

pub(super) fn seed_firefox_places(path: &std::path::Path) {
    let conn = Connection::open(path).expect("open places");
    conn.execute_batch(
        "CREATE TABLE moz_places (
            id INTEGER PRIMARY KEY,
            url TEXT
        );
        CREATE TABLE moz_historyvisits (
            id INTEGER PRIMARY KEY,
            place_id INTEGER NOT NULL
        );
        CREATE TABLE moz_bookmarks (
            id INTEGER PRIMARY KEY,
            fk INTEGER NOT NULL
        );
        CREATE TABLE moz_inputhistory (
            place_id INTEGER PRIMARY KEY
        );",
    )
    .expect("schema");
    conn.execute(
        "INSERT INTO moz_places(id, url) VALUES (1, ?1)",
        params!["https://keep.example.com/path"],
    )
    .expect("keep place");
    conn.execute(
        "INSERT INTO moz_places(id, url) VALUES (2, ?1)",
        params!["https://drop.example.com/path"],
    )
    .expect("drop place");
    conn.execute(
        "INSERT INTO moz_places(id, url) VALUES (3, ?1)",
        params!["https://bookmark-only.example.com/path"],
    )
    .expect("bookmark place");
    conn.execute(
        "INSERT INTO moz_historyvisits(id, place_id) VALUES (1, 1)",
        [],
    )
    .expect("keep visit");
    conn.execute(
        "INSERT INTO moz_historyvisits(id, place_id) VALUES (2, 2)",
        [],
    )
    .expect("drop visit");
    conn.execute("INSERT INTO moz_bookmarks(id, fk) VALUES (1, 3)", [])
        .expect("bookmark");
    conn.execute("INSERT INTO moz_inputhistory(place_id) VALUES (2)", [])
        .expect("inputhistory");
}

pub(super) fn read_i64(path: &std::path::Path, sql: &str) -> i64 {
    let conn = Connection::open(path).expect("open sqlite");
    conn.query_row(sql, [], |row| row.get(0)).expect("query")
}
