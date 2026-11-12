// LocationUrlComponents. Captured example values.
export default {
  "hostname": "www.target.com",
  "href": "https://www.target.com/login?client_id=ecom-web-1.0.0&ui_namespace=ui-default&back_button_action=bro",
  "port": "",
  "protocol": "https:"
};

// Illustrative: a build omits href.
export const hrefOmitted = {
  "hostname": "www.target.com",
  "port": "",
  "protocol": "https:"
};

// Illustrative: emulated location has missing hostname and port.
export const missingProperties = {
  "hostname": null,
  "href": "about:blank",
  "port": null,
  "protocol": "about:"
};

// Illustrative: a real about:blank page returns empty hostname and port strings, not null markers.
export const aboutBlank = {
  "hostname": "",
  "href": "about:blank",
  "port": "",
  "protocol": "about:"
};

// Captured from a second build of the collector; site/session identifier replaced with example.test.
export const capturedSecondBuild = {
  "hostname": "www.example.test",
  "href": "https://www.example.test/login",
  "port": "",
  "protocol": "https:"
};
