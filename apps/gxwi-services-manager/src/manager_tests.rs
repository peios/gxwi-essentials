use super::*;

fn summary(service: &str, display: Option<&str>, state: State) -> Summary {
    Summary {
        service: service.into(),
        display_name: display.map(Into::into),
        description: None,
        state,
        cause: None,
        health: None,
        next_timer_at: None,
    }
}

/// A window that has looked once and found three services, one of them
/// only in the registry, with the rights given.
fn manager(rights: &[(&str, Rights)]) -> Manager {
    let mut manager = Manager::new();
    manager.seen(Seen {
        services: Ok(vec![
            summary("timed", Some("Time client"), State::Active),
            summary("sshd", None, State::Inactive),
        ]),
        hidden: Hidden::These(vec![Defined::named("secret")]),
        rights: rights
            .iter()
            .cloned()
            .map(|(name, rights)| (name.to_string(), rights))
            .collect(),
        from: rights
            .iter()
            .filter(|(_, rights)| matches!(rights, Rights::Granted(_)))
            .map(|(name, _)| (name.to_string(), From::BuiltIn))
            .collect(),
        every: Ok(From::BuiltIn),
        every_changeable: Ok(()),
        changeable: None,
        creatable: Ok(()),
    });
    manager
}

fn shown(manager: &Manager, find: &str) -> String {
    let mut fields = Fields::default();
    fields.set("find", find);
    manager.render(&Facts {
        views: 1,
        fields: &fields,
    })
}

fn pick(manager: &mut Manager, service: &str) {
    manager.event(
        "pick",
        &serde_json::json!({ "service": service }),
        &mut Fields::default(),
    );
}

/// The labels and whether each is pressable, of the bar's commands.
fn bar(html: &str) -> Vec<(String, bool)> {
    let bar = html
        .split("<span class=\"commands\">")
        .nth(1)
        .unwrap()
        .split("</span>")
        .next()
        .unwrap();
    bar.split("</button>")
        .filter(|button| !button.is_empty())
        .map(|button| {
            (
                button.rsplit('>').next().unwrap().to_string(),
                !button.contains(" disabled"),
            )
        })
        .collect()
}

#[test]
fn services_are_listed_by_title_and_the_hidden_ones_say_so() {
    let manager = manager(&[]);
    let html = shown(&manager, "");
    let titles: Vec<&str> = html
        .split("<span class=\"name\">")
        .skip(1)
        .map(|rest| rest.split('<').next().unwrap())
        .collect();
    assert_eq!(titles, ["secret", "sshd", "Time client"]);
    assert!(html.contains("<span class=\"state\">Not yours to see</span>"));
    assert!(html.contains("2 services · 1 running"));
    assert!(html.contains("1 more whose state you may not see"));
    // Narrowed by what is typed, matched in the name or the title.
    let html = shown(&manager, "TIME");
    assert!(html.contains("Time client") && !html.contains("sshd"));
    assert!(html.contains("1 shown of 3"));
    assert!(shown(&manager, "nothing").contains("No service is called that."));
}

#[test]
fn where_the_registry_cannot_be_read_it_says_some_may_be_missing() {
    let mut manager = manager(&[]);
    manager.seen(Seen {
        services: Ok(Vec::new()),
        hidden: Hidden::Unknowable,
        rights: HashMap::new(),
        from: HashMap::new(),
        every: Ok(From::BuiltIn),
        every_changeable: Ok(()),
        changeable: None,
        creatable: Ok(()),
    });
    let html = shown(&manager, "");
    assert!(html.contains("Services whose state you may not see are not listed"));
    assert!(html.contains("There are no services you may see."));
}

#[test]
fn turned_away_by_the_service_manager_what_is_defined_is_listed_and_nothing_offered() {
    let mut manager = Manager::new();
    manager.seen(Seen {
        services: Err(Unasked::NotAllowed),
        hidden: Hidden::These(vec![
            Defined {
                name: "timed".into(),
                display_name: Some("Time client".into()),
                description: Some("Keeps the clock".into()),
            },
            Defined::named("sshd"),
        ]),
        rights: [("timed".to_string(), Rights::Granted(0xf))]
            .into_iter()
            .collect(),
        from: HashMap::new(),
        every: Err("you may not read who may control it".into()),
        every_changeable: Err("you may not change it".into()),
        changeable: None,
        creatable: Ok(()),
    });
    pick(&mut manager, "timed");
    manager.detailed(
        "timed",
        Err(Failure::Unreachable("connect: Permission denied".into())),
    );
    let html = shown(&manager, "");
    assert!(html.contains("The service manager does not let you ask it anything, so the state"));
    assert!(
        html.contains("2 services defined") && html.contains("Their state is not yours to see")
    );
    assert_eq!(
        html.matches("<span class=\"state\">Not yours to see</span>")
            .count(),
        2
    );
    // What the definition says it is called, and does, is shown where it can be read.
    assert!(
        html.contains("<span class=\"name\">Time client</span>")
            && html.contains("Keeps the clock")
    );
    assert!(html.contains("<span class=\"name\">sshd</span>"));
    // Whatever the descriptor says, peinit will not hear it.
    assert!(bar(&html).iter().all(|(_, pressable)| !pressable));
    assert!(!html.contains("Permission denied"));
}

#[test]
fn a_command_is_offered_only_with_its_right_and_where_it_does_something() {
    let all = Rights::Granted(0xf);
    let mut manager = manager(&[("timed", all.clone()), ("sshd", Rights::Granted(0x1))]);
    // Nothing is picked: nothing is pressable.
    assert!(
        bar(&shown(&manager, ""))
            .iter()
            .all(|(_, pressable)| !pressable)
    );
    pick(&mut manager, "timed");
    // Running: it can be stopped, restarted and reloaded, not started or reset.
    assert_eq!(
        bar(&shown(&manager, "")),
        [
            ("Start".into(), false),
            ("Stop".into(), true),
            ("Restart".into(), true),
            ("Reload".into(), true),
            ("Reset".into(), false)
        ]
    );
    let html = shown(&manager, "");
    assert!(html.contains("title=\"It is running already.\""));
    assert!(html.contains("You may start, stop and reload it."));
    // Only allowed to look: nothing is pressable, and it says why.
    pick(&mut manager, "sshd");
    let html = shown(&manager, "");
    assert!(bar(&html).iter().all(|(_, pressable)| !pressable));
    assert!(html.contains("title=\"You are not allowed to start it.\""));
    assert!(html.contains("You may not start, stop or reload it."));
    // Asked anyway, it is not sent.
    manager.event(
        "command",
        &serde_json::json!({ "command": "start", "service": "sshd" }),
        &mut Fields::default(),
    );
    assert_eq!(
        manager.said.as_ref().map(|said| said.text.as_str()),
        Some("sshd was not started: You are not allowed to start it.")
    );
    assert!(manager.working.is_empty());
}

#[test]
fn where_who_may_control_it_cannot_be_read_the_commands_are_offered() {
    let mut manager = manager(&[(
        "timed",
        Rights::Unknown("you may not read who may control it".into()),
    )]);
    pick(&mut manager, "timed");
    let html = shown(&manager, "");
    assert!(
        bar(&html)
            .iter()
            .any(|(label, pressable)| label == "Stop" && *pressable)
    );
    assert!(
        html.contains(
            "Whether you may control it is not known: you may not read who may control it."
        )
    );
}

#[test]
fn the_permissions_open_from_the_details_and_say_where_they_come_from() {
    let mut manager = manager(&[
        ("timed", Rights::Granted(0xf)),
        (
            "sshd",
            Rights::Unknown("you may not read who may control it".into()),
        ),
    ]);
    manager.from.insert("timed".into(), From::Own);
    pick(&mut manager, "timed");
    let html = shown(&manager, "");
    assert!(html.contains("Who may control it is set for this service."));
    assert!(html.contains(
        "fx-value-which=\"control\" fx-value-service=\"timed\">Who may control it…</button>"
    ));
    assert!(html.contains("fx-value-which=\"definition\" fx-value-service=\"timed\">Who may change its definition…</button>"));
    // Where who may control it cannot be read, that is not offered, and
    // why is said; the definition's own may still be.
    pick(&mut manager, "sshd");
    let html = shown(&manager, "");
    assert!(
        html.contains("Who may control it could not be read: you may not read who may control it.")
    );
    assert!(html.contains("fx-value-service=\"sshd\" disabled>Who may control it…"));
    assert!(html.contains("fx-value-service=\"sshd\">Who may change its definition…"));
    // A hidden service's are offered too: they are the registry's, not peinit's.
    pick(&mut manager, "secret");
    assert!(
        shown(&manager, "").contains("fx-value-service=\"secret\">Who may change its definition…")
    );
}

#[test]
fn what_was_set_goes_back_to_the_default_once_the_person_says_so() {
    let mut manager = manager(&[
        ("timed", Rights::Granted(0xf)),
        ("sshd", Rights::Granted(0xf)),
    ]);
    manager.from.insert("timed".into(), From::Own);
    pick(&mut manager, "timed");
    // Whether its definition may be changed is not known yet: the
    // button says so, unpressable.
    assert!(shown(&manager, "").contains(
        "disabled title=\"Whether you may has not been found out yet.\">Use the default</button>"
    ));
    manager.changeable = Some(("timed".into(), Ok(())));
    let html = shown(&manager, "");
    assert!(html.contains("fx-click=\"forget\" fx-value-which=\"service\" fx-value-service=\"timed\">Use the default</button>"));
    // Pressed, it asks, saying what it will take; kept, nothing is done.
    manager.event(
        "forget",
        &serde_json::json!({ "which": "service", "service": "timed" }),
        &mut Fields::default(),
    );
    let html = shown(&manager, "");
    assert!(html.contains("Time client will take the service manager's built-in default. Who may control it as set for it now will be lost."));
    assert!(html.contains("fx-click=\"forget-yes\"") && html.contains(">Keep its own</button>"));
    manager.event("forget-no", &serde_json::json!({}), &mut Fields::default());
    assert_eq!(manager.asking, None);
    // One taking the default already has nothing to go back to.
    pick(&mut manager, "sshd");
    assert!(!shown(&manager, "").contains("Use the default</button>"));
    // Every service: opened from any, and set back to the built-in default
    // only where something is set, and the person may.
    let html = shown(&manager, "");
    assert!(html.contains(
        "A service without permissions of its own takes the service manager's built-in default."
    ));
    assert!(html.contains(
        "fx-value-which=\"every\" fx-value-service=\"sshd\">Default permissions…</button>"
    ));
    assert!(!html.contains("Use the built-in default"));
    manager.every = Ok(From::AllServices);
    manager.every_changeable = Err("you may not change it".into());
    let html = shown(&manager, "");
    assert!(html.contains("takes the default set for every service."));
    assert!(
        html.contains(
            "disabled title=\"You may not change it.\">Use the built-in default</button>"
        )
    );
    // Asked for anyway, by a service with nothing of its own, nothing is asked.
    manager.event(
        "forget",
        &serde_json::json!({ "which": "service", "service": "sshd" }),
        &mut Fields::default(),
    );
    assert_eq!(manager.asking, None);
}

#[test]
fn what_came_of_a_command_is_said_in_words() {
    let mut manager = manager(&[("timed", Rights::Granted(0xf))]);
    manager.asked(Command::Restart, "timed", Outcome::Done(None));
    assert_eq!(
        manager.said,
        Some(Said {
            text: "Time client was restarted.".into(),
            bad: false
        })
    );
    manager.asked(
        Command::Reload,
        "timed",
        Outcome::Done(Some("reload signal confirmed".into())),
    );
    assert_eq!(
        manager.said,
        Some(Said {
            text: "Time client was reloaded.".into(),
            bad: false
        })
    );
    manager.asked(
        Command::Reload,
        "timed",
        Outcome::Done(Some(
            "reload signal advisory: detection window expired".into(),
        )),
    );
    assert_eq!(
        manager.said,
        Some(Said {
            text: "Time client was asked to reload, and did not say whether it had.".into(),
            bad: false
        })
    );
    manager.asked(
        Command::Stop,
        "timed",
        Outcome::Refused {
            code: "ACCESS_DENIED".into(),
            message: "access denied".into(),
        },
    );
    assert_eq!(
        manager.said,
        Some(Said {
            text: "You are not allowed to stop Time client.".into(),
            bad: true
        })
    );
    manager.asked(
        Command::Start,
        "sshd",
        Outcome::Failed("its program could not be found".into()),
    );
    assert_eq!(
        manager.said,
        Some(Said {
            text: "sshd could not be started: its program could not be found".into(),
            bad: true
        })
    );
}

#[test]
fn the_details_say_what_it_is_doing_and_why() {
    let mut manager = manager(&[("timed", Rights::Granted(0xf))]);
    manager.services[0].cause = Some("restart_policy".into());
    pick(&mut manager, "timed");
    manager.detailed(
        "timed",
        Ok(Status {
            summary: manager.services[0].clone(),
            status_text: Some("Synchronised".into()),
            job: Some(peinit::client::Job {
                id: "j".into(),
                pid: Some(412),
                started_at: None,
                identity: "S-1-5-19".into(),
            }),
            operation: None,
            uptime_seconds: Some(4000),
            definition_removed: false,
            warnings: Vec::new(),
            timers: Vec::new(),
            progress: None,
            granted: vec![
                "query_status".into(),
                "start".into(),
                "stop".into(),
                "interrogate".into(),
            ],
        }),
    );
    let html = shown(&manager, "");
    assert!(!html.contains("class=\"timers\""));
    assert!(html.contains("<dt>State</dt><dd>Running, restarted after it stopped</dd>"));
    assert!(html.contains("<dt>Process</dt><dd>412</dd>"));
    assert!(html.contains("<dt>Runs as</dt><dd>Local Service</dd>"));
    assert!(html.contains("<dt>Running for</dt><dd>1 h 6 min</dd>"));
    assert!(html.contains("<p class=\"id\">timed</p>"));
    // What it has logged opens in Event Viewer, from the details and its menu.
    assert!(html.contains("fx-click=\"logs\" fx-value-service=\"timed\">Logs…</button>"));
    assert!(html.contains("<li><button type=\"button\" fx-click=\"logs\">Logs…</button></li>"));
    // A hidden one says its state is not the person's, and is not asked
    // about again and again only to be refused.
    pick(&mut manager, "secret");
    assert_eq!(manager.picked(), None);
    let html = shown(&manager, "");
    assert!(html.contains("Its state is not yours to see."));
}

#[test]
fn a_definition_open_in_its_window_is_not_opened_again() {
    let mut manager = manager(&[("timed", Rights::Granted(0xf))]);
    pick(&mut manager, "timed");
    manager.defining.insert("timed".into());
    let html = shown(&manager, "");
    assert!(html.contains("fx-value-service=\"timed\" disabled title=\"Its definition is open already, in a window of its own.\">Definition…</button>"));
    assert!(
        html.contains(
            "<button type=\"button\" fx-click=\"definition\" disabled>Definition…</button>"
        )
    );
    // Asked for all the same, it is not opened.
    manager.event(
        "definition",
        &serde_json::json!({ "service": "timed" }),
        &mut Fields::default(),
    );
    assert_eq!(
        manager.said,
        Some(Said {
            text: "The definition of Time client is open already, in a window of its own.".into(),
            bad: false
        })
    );
}

#[test]
fn a_timer_says_when_it_runs_next_and_last_ran_or_why_it_never_does() {
    let mut manager = manager(&[("sshd", Rights::Granted(0xf))]);
    // Saturday 3 October 2026, 13:20, on a clock an hour east of UTC.
    manager.clock = words::Clock::Fixed(
        "2026-10-03T12:20:00Z".parse().unwrap(),
        jiff::tz::TimeZone::fixed(jiff::tz::offset(1)),
    );
    manager.services[1].next_timer_at = Some("2026-10-03T13:00:00.000000000Z".into());
    let html = shown(&manager, "");
    assert!(html.contains("<span>Next run</span>"));
    assert!(html.contains("<span class=\"next\">Today at 14:00</span>"));
    assert!(html.contains("<span class=\"next\"></span>"));
    pick(&mut manager, "sshd");
    let timer = |schedule: &str,
                 scheduled: Option<&str>,
                 fires: Option<&str>,
                 last: Option<&str>,
                 not_armed: Option<&str>| peinit::client::Timer {
        schedule: schedule.into(),
        scheduled_at: scheduled.map(Into::into),
        fires_at: fires.map(Into::into),
        last_fired_at: last.map(Into::into),
        not_armed: not_armed.map(Into::into),
    };
    manager.detailed(
        "sshd",
        Ok(Status {
            summary: manager.services[1].clone(),
            status_text: None,
            job: None,
            operation: None,
            uptime_seconds: None,
            definition_removed: false,
            warnings: Vec::new(),
            timers: vec![
                timer(
                    "*-*-* 01:00:00",
                    Some("2026-10-04T00:00:00.000000000Z"),
                    Some("2026-10-04T00:07:12.000000000Z"),
                    Some("2026-10-03T00:03:40.000000000Z"),
                    None,
                ),
                timer(
                    "hourly",
                    Some("2026-10-03T13:00:00.000000000Z"),
                    Some("2026-10-03T13:00:00.000000000Z"),
                    None,
                    None,
                ),
                timer(
                    "*-02-30",
                    None,
                    None,
                    None,
                    Some("calendar expression has no future occurrence"),
                ),
            ],
            progress: None,
            granted: vec![
                "query_status".into(),
                "start".into(),
                "stop".into(),
                "interrogate".into(),
            ],
        }),
    );
    let html = shown(&manager, "");
    assert!(html.contains("<h3>Timers</h3>"));
    assert!(html.contains("<code>*-*-* 01:00:00</code></p><dl><dt>Next run</dt><dd>Tomorrow at 01:07:12, in 11 h 47 min</dd><dt>Last run</dt><dd>Today at 01:03:40</dd></dl>"));
    assert!(html.contains("It is due tomorrow at 01:00, and put back by a random delay."));
    assert!(html.contains("<code>hourly</code></p><dl><dt>Next run</dt><dd>Today at 14:00, in 40 min</dd><dt>Last run</dt><dd>Not since the machine started</dd></dl></div>"));
    assert!(html.contains("<code>*-02-30</code></p><p class=\"note bad\">It never runs: calendar expression has no future occurrence.</p>"));
}
