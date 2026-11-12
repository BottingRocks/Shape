// UiEventSamplesTouch: illustrative data-bearing example.
// Illustrative touchstart, touchmove and touchend; changedTouches supplies the final point.
export default {
  "records": [
    {
      "time": 10,
      "points": [
        {
          "force": 0.5,
          "targetRelY": 20,
          "pageY": 120,
          "pageX": 100,
          "targetRelX": 10
        }
      ],
      "flags": {
        "isTouchmove": false,
        "isTouchstart": true,
        "isTrusted": true,
        "instanceOfUIEvent": true
      }
    },
    {
      "time": 12,
      "points": [
        {
          "force": null,
          "targetRelY": 25,
          "pageY": 125,
          "pageX": 105,
          "targetRelX": 15
        }
      ],
      "flags": {
        "isTouchmove": true,
        "isTouchstart": false,
        "isTrusted": true,
        "instanceOfUIEvent": true
      }
    },
    {
      "time": 15,
      "points": [
        {
          "force": 0,
          "targetRelY": 25,
          "pageY": 125,
          "pageX": 105,
          "targetRelX": 15
        }
      ],
      "flags": {
        "isTouchmove": false,
        "isTouchstart": false,
        "isTrusted": true,
        "instanceOfUIEvent": true
      }
    }
  ]
};

// Captured example: no records were recorded.
export const capturedEmpty = {
  "records": []
};

// Captured from a second build of the collector; decoded positional payload.
export const capturedSecondBuild = {
  "records": []
};
