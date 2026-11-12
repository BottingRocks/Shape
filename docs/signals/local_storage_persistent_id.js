// LocalStoragePersistentId. Captured example values.
export default {
  "storageStatus": 3,
  "storedId": "A7dnGmKdAQAAhZWcRWlWhl2h6vRTaGVX6Z9OSxaXqGmLDUfl8GFN27kLCff6AUm92wCucphhwH9eCOfvosJeCA=="
};

// Illustrative: localStorage is absent.
export const storageMissing = {
  "storageStatus": 1,
  "storedId": null
};

// Illustrative: saving the persistent ID failed.
export const storageWriteFailed = {
  "storageStatus": 2,
  "storedId": null
};

// Captured from a second build of the collector; persistent storage identifier replaced.
export const capturedSecondBuild = {
  "storageStatus": 3.0,
  "storedId": "example-local-storage-id"
};
