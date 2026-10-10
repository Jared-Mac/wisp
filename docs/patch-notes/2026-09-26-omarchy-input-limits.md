# Safer Omarchy connection

Wisp now rejects oversized or malformed daemon responses before they reach the
Omarchy shell. Chat, rooms, and account data have explicit size limits, and
incoming updates wait for the UI to finish processing the previous update.

The plugin requires Python 3. Existing accounts and servers remain compatible.
