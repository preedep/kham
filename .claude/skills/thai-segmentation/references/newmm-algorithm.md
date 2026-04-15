# newmm Algorithm Pseudocode

Dictionary-based maximal matching with TCC constraints.

## Core Algorithm

```
function segment(text, dict_trie):
    tcc_positions = compute_tcc_boundaries(text)
    n = len(text)
    
    // Build DAG: dag[i] = list of end positions reachable from i
    dag = HashMap<usize, Vec<usize>>
    
    for i in 0..n:
        if i not on char boundary: continue
        
        // Find all dictionary words starting at position i
        words = dict_trie.prefix_search(text[i..])
        for word in words:
            end = i + word.len()
            if end is valid TCC boundary:
                dag[i].push(end)
        
        // Fallback: single TCC as edge (for unknown words)
        next_tcc = next_tcc_boundary_after(i, tcc_positions)
        if next_tcc not already in dag[i]:
            dag[i].push(next_tcc)
    
    // Shortest path = fewest edges = fewest words
    path = shortest_path_bfs(dag, start=0, end=n)
    
    // Convert path to tokens
    tokens = []
    for (start, end) in path.windows(2):
        tokens.push(Token {
            text: text[start..end],
            span: start..end,
            kind: classify(text[start..end]),
        })
    
    return tokens
```

## Safe Mode

When DAG has too many edges at a position (ambiguity explosion):

```
SAFE_THRESHOLD = 100  // max edges per position

if dag[i].len() > SAFE_THRESHOLD:
    // Greedy: take longest match only
    dag[i] = [dag[i].max()]
```

## Shortest Path (BFS)

```
function shortest_path_bfs(dag, start, end):
    // BFS gives minimum number of edges (= minimum words)
    queue = deque([(start, [start])])
    visited = HashSet()
    
    while queue:
        (pos, path) = queue.popleft()
        if pos == end: return path
        if pos in visited: continue
        visited.add(pos)
        
        for next_pos in dag[pos]:
            queue.push_back((next_pos, path + [next_pos]))
    
    return [start, end]  // fallback: entire text as one token
```

## Token Classification

```
function classify(text):
    if all chars in Thai range: Thai
    if all chars are ASCII letters: Latin
    if all chars are digits (0-9 or ๐-๙): Number
    if all chars are whitespace: Whitespace
    if all chars are punctuation: Punctuation
    else: Unknown
```
