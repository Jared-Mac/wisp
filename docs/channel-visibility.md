# Server text-channel visibility

New text channels default to everyone on the server, including future accounts. Server admins can change any channel to Everyone, Server admins, or Selected members in Server settings. Owners and server admins always have access. Selected accounts need not be friends.

Migration 29 preserves the explicit audience of existing channels; it does not publish private history. Older create requests containing an explicit member list preserve that audience. Channel edits which omit visibility preserve existing permissions.

Visibility is separate from the signed encryption identity ledger. Content reads use current access permissions, including snapshots, history, pins, reactions, and attachments. Eligible accounts without an encryption identity can discover the channel immediately and receive signed admission after enrolling, through an available existing signer. New channels initialize enrolled server admins as signers.

Updated clients intersect ciphertext recipients with the current channel audience for messages, edits, attachments, and reactions. The server validates that recipient list against both the signed roster and current access. Older clients are refused when their full roster includes an account whose access has ended. Permission changes racing with a send fail closed. Regranting access preserves previously readable history while excluding messages sent to a different audience during the gap. Existing signatures and key pins remain intact.
