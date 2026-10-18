//! Package Manager: the software on this machine, what the repositories
//! offer, the updates waiting, what was changed before, and where software
//! comes from.
//!
//! peipkg is the only thing that changes what is installed, and it has no
//! daemon: Package Manager runs it, as the person, reading its queries'
//! JSON and holding a conversation with its driven mode for each change.

use libgxwi::App;

mod changes;
mod manager;
mod packages;
mod peipkg;
mod run;
mod sources;
mod store;
mod words;

use manager::Manager;

// What this program looks like, to whatever lists it. The icon itself is
// `gxwi-package-manager.svg` at the repo root, installed as the base theme's.
libgxwi::icon!(b"dev.peios.gxwi-package-manager");

fn main() {
    if std::env::args().nth(1).is_some() {
        eprintln!("gxwi-package-manager: usage: gxwi-package-manager (given {:?})", std::env::args().skip(1).collect::<Vec<_>>());
        std::process::exit(64);
    }
    let mut app = match App::connect() {
        Ok(app) => app,
        Err(e) => {
            eprintln!("gxwi-package-manager: no desktop to open on: {e}");
            std::process::exit(1);
        }
    };
    libgxwi::settings::stylesheet(&mut app);
    app.stylesheet("/gxwi-package-manager.css", include_str!("gxwi-package-manager.css"));
    let window = app.live("Package Manager", Manager::new());
    let aside = std::sync::Arc::downgrade(&window);
    window.update(|manager, _| {
        manager.window = aside;
        manager.reread();
        manager.check(false);
    });
    if let Err(e) = app.run() {
        eprintln!("gxwi-package-manager: {e}");
        std::process::exit(1);
    }
}
