# What is this project?

Imagine some program P(x) -> y where x is a string, and y is a boolean.
Assume that there exists some k-distinguishable DFA D that represents the language of all strings x where P(x) is true.

My project can take P(x) and k, and assuming k is correct, produce D.

### It's actually even better than that!

It can also take some program P'(x) -> y where y is also a string, alongside a goal DFA G.
Assume that there exists some k-distinguishable DFA D' that represents the language of all strings x where P(x) is is a member of G.

My project can take P'(x), G, and k, and assuming k is correct, produce D'.

### It's *actually* even better than *that!*

P(x) doesn't even have to be a deterministic program! There can be multiple possible instructions to execute at a certain point, and as long as there exist a set of instructions that allows P(x) to become a member of G, it will be a part of D.

# How do I use this?

The more general form of Turing machines (programs) this project uses are called String Rewriting Systems (SRSs).
SRSs are represented in the form:
```
1 1 0 - 0 0 1 #1 in example
0 1 1 - 1 0 0 #2 in example
# or more generally,
# LHS - RHS
```
The idea is given an input string S, any substring containing the LHS of a String Rewriting Rule can be turned into the RHS of that same rule. The rules above can be used to turn 1 1 0 1 into a string matching 0\*10\*:

| Before Rule Application | After Rule Application | Rule Used |
| - | - | - |
|**1 1 0** 1 | **0 0 1** 1 | #1 |
|0 **0 1 1** | 0 **1 0 0** | #2 |

The main window of SRS-to-DFA is a text editor where you can write your own SRS.

The rest should hopefully be somewhat inuitive -- give a goal DFA to the program, your best guess at what its k-distinguishability will be, pick a solver, and just run it.

[JFLAP](https://www.jflap.org/) is a recommended companion tool for this project. It is a tool to make and view DFAs (useful for building custom goals/viewing outputs).

# How does this work?

The underlying concept of this project is based on how DFA minimization works. To understand that in more detail, check out my talk I gave about it at SSU:

[![The title slide of my thesis](https://img.youtube.com/vi/RQNweqJN7Zw/0.jpg)](https://www.youtube.com/watch?v=RQNweqJN7Zw)

# Which solver should I use?

Subset is the fastest, but takes up more memory and only works on acyclic, length-preserving SRSs. Minkid is its inverse, working with all SRSs and being more compact, but running a little slower.

The legacy implementations are simpler ways of deducing the correctness of a string that involve no fancy tricks. Check out how impactful those fancy tricks have been:

legacy

BFS perf

![~1500 strings per second](https://github.com/demi-w/ssu-dfa-research/blob/main/gui/assets/BFS%20perf.png?raw=true)

Hash perf

![~750 strings per second](https://github.com/demi-w/ssu-dfa-research/blob/main/gui/assets/Hash%20perf.png?raw=true)

vs. recommended

Minkid perf

![~555000 strings per second](https://github.com/demi-w/ssu-dfa-research/blob/main/gui/assets/Minkid%20perf.png?raw=true)

Subset perf

![~35000 strings per second](https://github.com/demi-w/ssu-dfa-research/blob/main/gui/assets/Subset%20perf.png?raw=true)

To highlight the strings deduced per second, my recommended solvers are ~740x faster. Additionally, tasks that take BFS and Hash solvers ~128GB of memory take Minkid and Subset ~40MB. Additionally, BFS is the only implementation that uses multithreading, so that's 16 threads of BFS getting blown out by a single thread of Minkid.

More to come on the details of these implementations, but for now, just know that both methods exploit single-SRS-application connections between states in the partially completed DFA. (This is referred to as the rule/link graph in the codebase.)

# Does the GUI cause solving to be slower?

No, as the solver is run on a different thread, and summary information is simply passed over to the GUI.

# What do you hope to do with this?

Initially, this was just my CS496 Senior Thesis project, however my professor and I both wanted to take it further (even though I've graduated!). We're planning to submit this work to automata conferences ~Summer 2024.