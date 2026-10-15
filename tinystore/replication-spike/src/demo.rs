use std::path::Path;
use std::sync::{Arc, Mutex};

use tinystore::{Options, Store};

use crate::node::{Limits, Node};
use crate::outbox::Outbox;
use crate::transport::{self, Server};
use crate::{Result, require};

pub(super) fn run(root: &Path) -> Result<()> {
    require(!root.join("us").exists(), "demo needs a fresh directory")?;
    let (us_store, mut us) = primary(&root.join("us"), "us-app", "American primary")?;
    let (eu_store, mut eu) = primary(&root.join("eu"), "eu-local", "European database")?;
    let mut ru = Node::open(&root.join("ru"))?;
    let mut embedded = Node::open(&root.join("embedded"))?;
    let us = Arc::new(Mutex::new({
        us.capture("us-app")?;
        us
    }));
    eu.capture("eu-local")?;
    let us_server = Server::start(Arc::clone(&us))?;
    transport::sync(&mut eu, us_server.address, "us-app", Limits::default())?;
    let eu = Arc::new(Mutex::new(eu));
    let eu_server = Server::start(Arc::clone(&eu))?;
    transport::follow(&mut ru, eu_server.address, Limits::default())?;
    let ru = Arc::new(Mutex::new(ru));
    let ru_server = Server::start(Arc::clone(&ru))?;
    transport::follow(&mut embedded, ru_server.address, Limits::default())?;
    println!("US -> EU (+ own database) -> RU -> embedded: both streams searchable locally");
    require(
        search(&embedded, "us-app", "American")? == 1,
        "embedded search missed primary data",
    )?;
    require(
        search(&embedded, "eu-local", "European")? == 1,
        "embedded search missed relay's own data",
    )?;
    fan_out(root, &eu_server)?;
    eu.lock().unwrap().prune("us-app", 1)?;
    let mut late = Node::open(&root.join("late"))?;
    transport::sync(&mut late, eu_server.address, "us-app", Limits::default())?;
    require(
        search(&late, "us-app", "American")? == 1,
        "snapshot recovery failed",
    )?;
    println!("Pruned relay history: a late child bootstrapped from the current snapshot");
    queued_intent(root, &us_store, &us)?;
    resume_after_outage(&us, &eu, &us_store, us_server)?;
    transport::sync(
        &mut ru.lock().unwrap(),
        eu_server.address,
        "us-app",
        Limits::default(),
    )?;
    transport::sync(
        &mut embedded,
        ru_server.address,
        "us-app",
        Limits::default(),
    )?;
    require(
        search(&embedded, "us-app", "queued")? == 1,
        "backlog did not reach embedded copy",
    )?;
    // The old writer is closed before the manual promotion, not merely hidden.
    us_store.close()?;
    eu.lock().unwrap().promote("us-app")?;
    eu.lock().unwrap().write(
        "us-app",
        "insert into documents values(3, 'standby promoted')",
    )?;
    transport::sync(
        &mut ru.lock().unwrap(),
        eu_server.address,
        "us-app",
        Limits::default(),
    )?;
    transport::sync(
        &mut embedded,
        ru_server.address,
        "us-app",
        Limits::default(),
    )?;
    require(
        search(&embedded, "us-app", "promoted")? == 1,
        "promoted writes did not cascade",
    )?;
    println!("Fenced US, promoted EU stream to epoch 2: new writes reached RU and embedded");
    eu_store.close()?;
    println!("PASS: persistent state in {}", root.display());
    Ok(())
}

fn queued_intent(root: &Path, primary: &Store, node: &Mutex<Node>) -> Result<()> {
    let path = root.join("embedded/outbox.db");
    let queue = Outbox::open(&path)?;
    queue.enqueue(
        "embedded/request-1",
        "insert into documents values(10, 'offline intent')",
    )?;
    require(
        queue
            .drain(|_, _| Err(std::io::Error::other("primary unavailable").into()))
            .is_err(),
        "offline delivery unexpectedly succeeded",
    )?;
    drop(queue);
    let queue = Outbox::open(&path)?;
    require(
        queue.pending()? == 1,
        "pending intent did not survive reopen",
    )?;
    let db = primary.database("app").open()?;
    require(
        queue
            .drain(|id, sql| {
                Outbox::apply(&db, id, sql)?;
                Err(std::io::Error::other("answer lost after commit").into())
            })
            .is_err(),
        "lost-answer injection did not fail",
    )?;
    queue.drain(|id, sql| Outbox::apply(&db, id, sql))?;
    require(queue.pending()? == 0, "outbox did not acknowledge delivery")?;
    node.lock().unwrap().capture("us-app")?;
    println!(
        "SDK outbox: pending intent survived reopen and a lost answer; primary applied it once"
    );
    Ok(())
}

fn primary(root: &Path, stream: &str, text: &str) -> Result<(Store, Node)> {
    let store = Store::open(
        root,
        Options {
            background: false,
            ..Options::default()
        },
    )?;
    let db = store.database("app").open()?;
    db.exec("create table documents(id integer primary key, title text not null)")?;
    db.exec(tinystore::sql::Sql::new("insert into documents values(1, ?)").bind(text))?;
    let node = Node::open(root)?;
    node.publish(stream, "sql/app.db")?;
    Ok((store, node))
}

fn search(node: &Node, stream: &str, text: &str) -> Result<u32> {
    node.read(stream, |db| {
        Ok(db.query_row(
            "select count(*) from documents where title like '%' || ?1 || '%'",
            [text],
            |row| row.get(0),
        )?)
    })
}

fn fan_out(root: &Path, parent: &Server) -> Result<()> {
    for index in 0..140 {
        let mut child = Node::open(&root.join(format!("children/{index}")))?;
        for stream in ["us-app", "eu-local"] {
            transport::sync(&mut child, parent.address, stream, Limits::default())?;
        }
        require(
            search(&child, "us-app", "American")? == 1,
            "child missed primary data",
        )?;
        require(
            search(&child, "eu-local", "European")? == 1,
            "child missed relay data",
        )?;
    }
    println!("140 child directories pulled both streams over TCP, with bounded batches");
    Ok(())
}

fn resume_after_outage(
    us: &Arc<Mutex<Node>>,
    eu: &Arc<Mutex<Node>>,
    store: &Store,
    server: Server,
) -> Result<()> {
    let address = server.address;
    drop(server);
    store
        .database("app")
        .open()?
        .exec("insert into documents values(2, 'queued while link was down')")?;
    us.lock().unwrap().capture("us-app")?;
    require(
        transport::sync(
            &mut eu.lock().unwrap(),
            address,
            "us-app",
            Limits::default(),
        )
        .is_err(),
        "disconnected sync unexpectedly succeeded",
    )?;
    require(
        search(&eu.lock().unwrap(), "us-app", "American")? == 1,
        "offline replica cannot read",
    )?;
    let restarted = Server::start(Arc::clone(us))?;
    transport::sync(
        &mut eu.lock().unwrap(),
        restarted.address,
        "us-app",
        Limits::default(),
    )?;
    require(
        search(&eu.lock().unwrap(), "us-app", "queued")? == 1,
        "durable feed did not resume",
    )?;
    println!("Upstream unavailable: local reads kept working; reconnect resumed the retained feed");
    Ok(())
}
