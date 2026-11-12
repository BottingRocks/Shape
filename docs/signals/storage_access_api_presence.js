// StorageAccessApiPresence: captured example.
export default {
  "getHostEnvironmentValue": false,
  "hasStorageAccess": true,
  "requestStorageAccess": true
};

// Illustrative: the legacy external API is present; the Storage Access API methods are not.
export const legacyExternalApi = {
  "getHostEnvironmentValue": true,
  "hasStorageAccess": false,
  "requestStorageAccess": false
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "getHostEnvironmentValue": false,
  "hasStorageAccess": true,
  "requestStorageAccess": true
};
