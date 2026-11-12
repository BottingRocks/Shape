// BrowserApiProxyFlags: captured example values.
export default {
  "proxyChecks": [
    {
      "CanvasRenderingContext2D.prototype.getImageData": false,
      "Function.prototype.toString": false,
      "HTMLCanvasElement.prototype.toDataURL": false,
      "Intl.supportedValuesOf": false,
      "Navigator.prototype.hardwareConcurrency": false,
      "Proxy": true,
      "navigator.mimeTypes": false
    },
    {
      "HTMLMediaElement.prototype.canPlayType": false,
      "Intl": false,
      "Navigator.prototype.languages": false,
      "Navigator.prototype.plugins": false,
      "Permissions.prototype.canPlayType": false,
      "document.createElement": false,
      "navigator.getBattery": false
    },
    {
      "WebGLRenderingContext.prototype.getParameter": false,
      "navigator": false,
      "navigator.plugins": false,
      "speechSynthesis": false
    }
  ]
};

// Illustrative tampered environment: canvas export and Function.prototype.toString behave like Proxies.
export const wrappedApis = {
  "proxyChecks": [
    {
      "CanvasRenderingContext2D.prototype.getImageData": false,
      "Function.prototype.toString": true,
      "HTMLCanvasElement.prototype.toDataURL": true,
      "Intl.supportedValuesOf": false,
      "Navigator.prototype.hardwareConcurrency": false,
      "Proxy": true,
      "navigator.mimeTypes": false
    },
    {
      "HTMLMediaElement.prototype.canPlayType": false,
      "Intl": false,
      "Navigator.prototype.languages": false,
      "Navigator.prototype.plugins": false,
      "Permissions.prototype.canPlayType": false,
      "document.createElement": false,
      "navigator.getBattery": false
    },
    {
      "WebGLRenderingContext.prototype.getParameter": false,
      "navigator": false,
      "navigator.plugins": false,
      "speechSynthesis": false
    }
  ]
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "proxyChecks": [
    {
      "CanvasRenderingContext2D.prototype.getImageData": false,
      "Function.prototype.toString": false,
      "HTMLCanvasElement.prototype.toDataURL": false,
      "Intl.supportedValuesOf": false,
      "Navigator.prototype.hardwareConcurrency": false,
      "Proxy": true,
      "navigator.mimeTypes": false
    },
    {
      "HTMLMediaElement.prototype.canPlayType": false,
      "Intl": false,
      "Navigator.prototype.languages": false,
      "Navigator.prototype.plugins": false,
      "Permissions.prototype.canPlayType": false,
      "document.createElement": false,
      "navigator.getBattery": false
    },
    {
      "WebGLRenderingContext.prototype.getParameter": false,
      "navigator": false,
      "navigator.plugins": false,
      "speechSynthesis": false
    }
  ]
};
