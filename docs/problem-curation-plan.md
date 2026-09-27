# Interview problem curation plan

## Goal and scope

Build a catalog of **125 interview practice problems**: the four current catalog entries plus **121 new problems**. The target mix is **24 easy, 61 medium, 35 hard, and 5 extra-hard**. The first 100 form a breadth-first curriculum across common data-structure and algorithm patterns; the final 25 add advanced practice.

The numbered items below are planning IDs, not catalog keys. Titles are working titles; each published statement and test suite should be written in our own words. Preserve the four existing problems and their keys unless a separate review finds a defect. Publish only complete, reviewed artifacts.

## Backlog

`E` = easy, `M` = medium, `H` = hard, `XH` = extra-hard. `✓` marks an existing catalog problem. Extra-hard is a curation label: publish it as `difficulty: "hard"` with `difficulty_score: 5`. Publish the 20 additional hard problems with `difficulty_score: 4`.

### Arrays and matrices (12)

| ID | Level | Working title / core pattern |
| --- | --- | --- |
| 001 | E | ✓ Between the Markers — comparisons and boundary order |
| 002 | E | ✓ Quiet Peaks — neighbor scan and edge cases |
| 003 | M | ✓ Folded Rows — ragged matrix transformation |
| 004 | E | Stable Sign Partition — stable array filtering |
| 005 | E | Rotate a Sequence — modular indices |
| 006 | M | Product Except Position — prefix and suffix products |
| 007 | E | Merge Ordered Runs — merge two sorted sequences |
| 008 | M | Spiral Matrix Readout — directional boundaries |
| 009 | M | Clear Marked Rows and Columns — matrix markers |
| 010 | H | First Missing Positive — index placement |
| 011 | H | Rainwater Between Bars — two-sided bounds |
| 012 | M | Maximum Contiguous Sum — running optimum |

### Hashing and prefix sums (8)

| ID | Level | Working title / core pattern |
| --- | --- | --- |
| 013 | E | Pair Sum Indices — hash lookup |
| 014 | E | First Unique Value — frequencies and stable order |
| 015 | M | Group Rearrangements — canonical string keys |
| 016 | M | Longest Consecutive Span — set membership |
| 017 | M | Target Sum Subarrays — prefix-sum counts |
| 018 | M | Equal Binary Span — transformed prefix sums |
| 019 | M | Distinct Values per Window — rolling frequencies |
| 020 | M | Four-List Sum Count — pair-sum hashing |

### Strings and sliding windows (9)

| ID | Level | Working title / core pattern |
| --- | --- | --- |
| 021 | E | Alphanumeric Palindrome — normalization and two pointers |
| 022 | M | Longest Unique Substring — moving left boundary |
| 023 | H | Smallest Covering Substring — required frequencies |
| 024 | M | Permutation Windows — fixed-size frequency window |
| 025 | M | Longest Uniform Replacement — window invariant |
| 026 | E | Reverse Word Order — token handling |
| 027 | E | Compress Consecutive Characters — run counting |
| 028 | M | Expand Nested Repeats — nested parsing |
| 029 | E | Shared Prefix — prefix scan |

### Two pointers and sorting (7)

| ID | Level | Working title / core pattern |
| --- | --- | --- |
| 030 | E | Sorted Pair Sum — inward pointers |
| 031 | M | Three Values to Target — sort and sweep |
| 032 | M | Maximum Water Container — greedy pointer move |
| 033 | E | Squares in Sorted Order — merge from both ends |
| 034 | M | Three-Color Partition — Dutch-flag partition |
| 035 | H | Inversion Count — merge-sort counting |
| 036 | M | Smallest Unsorted Span — boundary expansion |

### Stacks and queues (6)

| ID | Level | Working title / core pattern |
| --- | --- | --- |
| 037 | E | Balanced Delimiters — stack matching |
| 038 | M | Postfix Expression — operand stack |
| 039 | M | Minimum Stack — stateful stack design |
| 040 | M | Next Warmer Day — monotonic stack |
| 041 | H | Maximum in Every Window — monotonic deque |
| 042 | H | Largest Histogram Rectangle — monotonic boundaries |

### Binary search (6)

| ID | Level | Working title / core pattern |
| --- | --- | --- |
| 043 | E | First Position at Least Target — lower bound |
| 044 | M | Find in Rotated Order — split sorted range |
| 045 | M | Minimum in Rotated Order — binary-search invariant |
| 046 | H | Median of Two Sorted Sequences — partition search |
| 047 | M | Smallest Feasible Capacity — search over answers |
| 048 | H | Kth Pair Distance — count under threshold |

### Trees and binary search trees (10)

| ID | Level | Working title / core pattern |
| --- | --- | --- |
| 049 | E | Tree Height — recursion |
| 050 | E | Mirror a Binary Tree — structural transformation |
| 051 | M | Values by Level — breadth-first traversal |
| 052 | M | Valid Search Tree — ancestor bounds |
| 053 | M | Lowest Shared Ancestor — recursive search |
| 054 | M | Kth Search-Tree Value — inorder traversal |
| 055 | M | Build Tree from Traversals — index map and recursion |
| 056 | M | Longest Tree Path — diameter |
| 057 | E | Height-Balanced Tree — bottom-up validation |
| 058 | H | Maximum Tree Path Sum — local and global gains |

### Linked lists (6)

| ID | Level | Working title / core pattern |
| --- | --- | --- |
| 059 | E | Reverse a List — pointer reversal |
| 060 | E | Merge Sorted Lists — sentinel and merge |
| 061 | M | Remove the Nth Node from the End — two pointers |
| 062 | M | Reorder List Ends — split, reverse, weave |
| 063 | M | Add List-Encoded Integers — carry propagation |
| 064 | H | Merge K Sorted Lists — heap or divide and conquer |

### Heaps and intervals (7)

| ID | Level | Working title / core pattern |
| --- | --- | --- |
| 065 | M | Most Frequent K Values — frequency ranking |
| 066 | E | K Largest Values — bounded heap |
| 067 | H | Running Median — two heaps, stateful interface |
| 068 | M | Minimum Concurrent Rooms — sorted endpoints or heap |
| 069 | M | Insert an Interval — overlap merging |
| 070 | M | Fewest Arrows for Intervals — sorted endpoints |
| 071 | M | Merge K Sorted Arrays — k-way heap |

### Graphs (10)

| ID | Level | Working title / core pattern |
| --- | --- | --- |
| 072 | M | Count Land Components — grid traversal |
| 073 | E | Recolor a Region — flood fill |
| 074 | M | Shortest Clear Grid Route — BFS |
| 075 | M | Course Ordering — topological sort |
| 076 | M | Two-Color Graph — bipartite check |
| 077 | M | Copy a Graph — identity-preserving traversal |
| 078 | M | Minimum Weighted Distances — Dijkstra |
| 079 | M | Minimum Connection Cost — spanning tree |
| 080 | H | Offline Connectivity Queries — union-find and reversal |
| 081 | H | Shortest Word Transform — implicit graph BFS |

### Dynamic programming (9)

| ID | Level | Working title / core pattern |
| --- | --- | --- |
| 082 | E | Staircase Ways — one-dimensional recurrence |
| 083 | M | Maximum Nonadjacent Sum — take/skip recurrence |
| 084 | M | Minimum Coins for Amount — unbounded choices |
| 085 | M | Longest Increasing Subsequence — ordered tails |
| 086 | M | Longest Shared Subsequence — two-dimensional DP |
| 087 | M | Grid Route Count — spatial recurrence |
| 088 | M | Equal-Sum Partition — subset states |
| 089 | H | Minimum Edit Operations — alignment DP |
| 090 | M | Number String Decodings — prefix recurrence |

### Backtracking (5)

| ID | Level | Working title / core pattern |
| --- | --- | --- |
| 091 | M | All Subsets — include/exclude recursion |
| 092 | M | Unique Permutations — duplicate pruning |
| 093 | M | Sum Combinations — branch pruning |
| 094 | M | Find a Word in a Grid — path search |
| 095 | H | Place N Queens — column and diagonal constraints |

### Greedy, bits, and design (5)

| ID | Level | Working title / core pattern |
| --- | --- | --- |
| 096 | M | Reach the Final Index — greedy reachability |
| 097 | M | Complete Fuel Circuit — greedy restart |
| 098 | E | Single Unpaired Value — XOR |
| 099 | M | Prefix Dictionary — stateful trie |
| 100 | M | ✓ LRU Cache — hash map and recency order |

### Advanced extension (25)

These 20 hard and 5 extra-hard problems extend the tracks above. Keep their primary topic in the artifact tags so they remain discoverable alongside the core problems.

| ID | Level | Primary topic | Working title / core pattern |
| --- | --- | --- | --- |
| 101 | H | Sorting | Count Smaller Values to the Right — merge-sort counting |
| 102 | H | Greedy | Minimum Candy Allocation — two directional passes |
| 103 | H | Stack | Longest Valid Parenthesis Span — stack or DP |
| 104 | H | Backtracking | Remove the Fewest Invalid Parentheses — level-order search |
| 105 | H | Dynamic programming | Regular Expression Matching — `.` and `*` states |
| 106 | H | Dynamic programming | Wildcard Pattern Matching — `?` and `*` states |
| 107 | H | Dynamic programming | Minimum Palindrome Cuts — palindrome precomputation |
| 108 | H | Strings | Shortest Prefix Palindrome — prefix-function matching |
| 109 | H | Binary search | Split an Array by Largest Segment Sum — feasibility search |
| 110 | H | Prefix sums | Count Sums in an Inclusive Range — divide-and-conquer counting |
| 111 | H | Matrices | Largest All-One Rectangle — histogram reduction |
| 112 | H | Strings | All Valid Word Breaks — memoized sentence generation |
| 113 | H | Trees | Place the Fewest Tree Cameras — tree-state DP |
| 114 | H | Trees | Repair a Swapped Search Tree — inorder anomaly detection |
| 115 | H | Heaps | Kth Value in a Sorted Matrix — heap or value-domain search |
| 116 | H | Graphs | Infer an Alien Alphabet — topological order and invalid-prefix detection |
| 117 | H | Graphs | Critical Network Connections — low-link DFS |
| 118 | H | Graphs | Cheapest Route with a Stop Limit — layered shortest paths |
| 119 | H | Heaps | Sliding Window Median — dual heaps with lazy deletion |
| 120 | H | Dynamic programming | Three Nonoverlapping Maximum-Sum Windows — prefix sums and DP |
| 121 | XH | Intervals | City Skyline — sweep line with active heights |
| 122 | XH | Tries | Find Dictionary Words in a Grid — trie-guided backtracking |
| 123 | XH | Dynamic programming | Maximum Coins from Bursting Balloons — interval DP |
| 124 | XH | Graphs | Visit Every Graph Node — bitmask-state BFS |
| 125 | XH | Dynamic programming | Count Distinct Palindromic Subsequences — duplicate-aware interval DP |

## Authoring and review workflow

The initial 125-problem curation is complete. For new or revised entries, copy the sibling repository's `example-problem/` into `problems/<slug>/`, then edit `problem.yaml`, `statement.md`, ordered hint files, and one case per YAML or JSON file. The catalog discovers these files without a manifest. See [Git-native problem authoring](git-problem-authoring-plan.md).

Fix the interface, constraints, return ordering, tie rules, empty inputs, overflow behavior, and intended complexity before writing cases. Give each new problem at least two visible examples and hidden boundary, degenerate, tie, and scale cases where applicable. A reviewer checks that the statement determines one result, examples match the contract, and tags and difficulty are appropriate. Run `catalog-validate` and the quality and oracle checks before merge; run every present reference against all cases in the gVisor sandbox.

## Completion criteria

- Exactly 125 unique catalog keys and valid assembled schema-3 problems, including the four original entries.
- All 13 core tracks and the 25 advanced entries retain the planned mix: 24 easy, 61 medium, 35 hard at score 4, and 5 extra-hard at score 5.
- Visible and hidden cases, their order, and expected outputs remain unchanged after source-format migration.
- Present reference solutions pass all cases; references remain optional for the migrated catalog.

## Current implementation status

All 125 problems have been converted to directory sources. Their assembled metadata, statements, and ordered cases match the original JSON artifacts. The quality gate and independent fixture oracles remain in place. Reference coverage can grow over time.
