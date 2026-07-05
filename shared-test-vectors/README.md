Shared test vectors pin deterministic client logic that is implemented in more
than one runtime.

Each JSON file contains named cases consumed directly by the relevant platform
unit tests. When a behavior changes, update the vector first and make every
client match it in the same PR.
