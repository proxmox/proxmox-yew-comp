# Dialog browser regression tests

These tests mount the real PWT dialogs, EditWindow, and Wizard in Chromium. They
exercise native modal state, retained input and wizard pages, close notifications,
keyboard activation, confirmation cancellation, pointer/touch dismissal, and
inline-error layout. Screenshots and the built application are retained under
`build/` for each run.

Requirements: Rust with the wasm32-unknown-unknown target, trunk, Chromium, and a
compiled PWT theme, a FontAwesome stylesheet, and their font assets. Cargo uses the
repository's configured sources. To test an unreleased toolkit, configure a Cargo
patch for `pwt` pointing to that checkout.

```sh
cd tests/browser
npm install
PWT_TEST_CSS=/path/to/desktop-yew-style.css npm test
```

`PWT_TEST_ASSETS` selects the asset root if it is not the stylesheet's directory.
It must contain `font-awesome.css` (a hashed filename also works) and `fonts/`.
`CHROMIUM` overrides `/usr/bin/chromium`. `PWT_TEST_FILTER` runs only case names
containing the supplied string. Run with desktop, material, and mobile stylesheets
to check their different spacing and dialog styles. These are component
integration tests, not a replacement for application E2E tests.
