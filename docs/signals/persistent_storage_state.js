// persistent_storage_state: Illustrative startup; no inherited ID.
export default {
  "removedValue": null,
  "flags": {
    "storageAccessFailed": false,
    "noInheritedId": true,
    "idRemovalSeen": false
  }
};

// Illustrative removal/clear event with the removed ID.
export const idRemoved = {
  "removedValue": "example-persistent-id",
  "flags": {
    "storageAccessFailed": false,
    "noInheritedId": false,
    "idRemovalSeen": true
  }
};

// Illustrative startup when storage access throws.
export const storageUnavailable = {
  "removedValue": null,
  "flags": {
    "storageAccessFailed": true,
    "noInheritedId": true,
    "idRemovalSeen": false
  }
};

// Illustrative: startup on a returning visit, with the stored ID inherited and no removal seen.
export const inheritedId = {
  "removedValue": null,
  "flags": {
    "storageAccessFailed": false,
    "noInheritedId": false,
    "idRemovalSeen": false
  }
};
