# TODO

- [ ] Import milli core crate (currently out of date on crates.io)
- [ ] Periodic re-indexation of pages. Keep search online during re-index.
- [ ] Re-index individual files when changed via debounced notify crate.
      (reindex all pages for now as simpler and still fast)
- [ ] Add main navigation menu and header/sidebar.
- [ ] Use lol_html to prettify HTML before sending response.
- [ ] Fetch only fragment when requesting a new page, to prevent full page
      reload.
- [ ] Add templating support to allow end users to create/tweak themes.
- [ ] Build a sitemap.
- [ ] Add publish date to frontmatter format, and return 404 for unbublished
      pages.
- [ ] Add frontmatter field for listing child pages on the page, allows
      blog/news like sections where the user chooses to have them.
- [ ] Add authentication module, allow signed in users to create and change page
      content.
- [ ] Setup comments system for pages that allows comments. Challenge is to
      allow comments without authentication, with optional memory of nicname.
      it also muse be spam free.
- [ ] Server sent events to notify user if the page viewing has changed to
      refresh that page.
- [ ] Localization and internationalization
- [ ] Guided HTTPS setup, likely via letsencrypt
