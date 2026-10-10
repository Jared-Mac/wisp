# Transfer progress cleanup

- Fixed a memory-growth issue in the desktop and Omarchy plugin when receiving
  repeated file-transfer updates. Progress entries now have a fixed limit,
  expire after completion or inactivity, and clear on disconnect.
