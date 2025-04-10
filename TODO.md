# TODO

- [ ] Periodic re-indexation of pages. Keep search online during re-index.
- [ ] Re-index individual files when changed via debounced notify crate.
- [ ] Fetch only fragment when requesting a new page, to prevent full page
      reload.
- [ ] Use crate askama for templating.
- [ ] Use lol_html to prettify HTML before sending response.
- [ ] Setup comments system for pages that allows comments. Challenge is to
      allow comments without authentication, with optional memory of nicname.
      it also muse be spam free. 
- [ ] Server sent events to notify user if the page viewing has changed to
      refresh that page.
