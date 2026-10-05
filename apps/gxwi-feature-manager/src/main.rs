//! Feature Manager: the features of this machine, what each is, and
//! turning them on and off.
//!
//! feat is the only thing that changes a feature, and it has no daemon:
//! Feature Manager runs it, as the person, reading its list's JSON and the
//! events of its driven mode for each change.

use libgxwi::App;

mod feat;
mod manager;
mod page;

use manager::Manager;

// What this program looks like, to whatever lists it. The icon itself is
// `gxwi-feature-manager.svg` at the repo root, installed as the base theme's.
libgxwi::icon!(b"dev.peios.gxwi-feature-manager");

fn main() {
    if std::env::args().nth(1).is_some() {
        eprintln!("gxwi-feature-manager: usage: gxwi-feature-manager (given {:?})", std::env::args().skip(1).collect::<Vec<_>>());
        std::process::exit(64);
    }
    let mut app = match App::connect() {
        Ok(app) => app,
        Err(e) => {
            eprintln!("gxwi-feature-manager: no desktop to open on: {e}");
            std::process::exit(1);
        }
    };
    libgxwi::settings::stylesheet(&mut app);
    app.stylesheet("/gxwi-feature-manager.css", include_str!("gxwi-feature-manager.css"));
    let window = app.live("Feature Manager", Manager::new());
    let aside = std::sync::Arc::downgrade(&window);
    window.update(|manager, _| {
        manager.window = aside;
        manager.reread();
    });
    if let Err(e) = app.run() {
        eprintln!("gxwi-feature-manager: {e}");
        std::process::exit(1);
    }
}
