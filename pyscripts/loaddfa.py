import json
from automata.fa.dfa import DFA


def loadFromFile(filename : str) -> DFA:
    with open(filename) as f:
        j = json.loads(f.read())
        newTransitions = {}
        for i in range(len(j["state_transitions"])):
            dest = {}
            for k in range(j["symbol_set"]["length"]):
                dest[str(k)] = 'q' + str(j["state_transitions"][i][k])
            newTransitions['q' + str(i)] = dest
        dfa = DFA(
        states=set(['q'+str(i) for i in range(len(j["state_transitions"]))]),
        input_symbols=set([str(i) for i in range(j["symbol_set"]["length"])]),
        transitions=newTransitions,
        initial_state='q' + str(j["starting_state"]),
        final_states=set(['q' + str(i) for i in j["accepting_states"]])
        )
    return dfa

dfa = loadFromFile("default2dpegx3.dfa")
print("Is 261 in the 3xk language: ", dfa.accepts_input('261'))
print("Is 101 in the 3xk language: ", dfa.accepts_input('101'))

dfa = loadFromFile("default1dpeg.dfa")
print("Is 1101 in the 1d language: ", dfa.accepts_input('1101'))
print("Is 101 in the 1d language: ", dfa.accepts_input('101'))
