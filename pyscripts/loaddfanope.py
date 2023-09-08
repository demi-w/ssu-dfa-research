import json
class DFA:
    def __init__(self, symbolReps : list[str], startingState : int, transTable : list[list[int]], acceptingStates : list[int]):
        self.symbolReps = symbolReps
        self.startingState = startingState
        self.transTable = transTable
        self.acceptingStates = acceptingStates
    def fromJSON(json):
        dfa = DFA(json["symbol_set"]["representations"], json["starting_state"], json["state_transitions"], json["accepting_states"])
        return dfa

    #Determines if a string is in the 
    def contains_from_idxs(self, inputString : list[int]) -> bool:
        curState = self.startingState
        for i in inputString:
            curState = self.transTable[curState][i]
        return curState in self.acceptingStates

    #Converts each symbol's string representation to that symbol's index
    def contains(self, inputString : list[str]) -> bool:
        for i in range(len(inputString)):
            inputString[i] = self.symbolReps.index(inputString[i])
        return self.contains_from_idxs(inputString)

with open("1dpeg.dfa") as f:
    s = json.loads(f.read())
    dfa = DFA.fromJSON(s)

print("Is 110011 in the language: ", dfa.contains(["1","1","0","0","1","1"]))
print("Is the empty string in the language: ", dfa.contains([]))
print("Is 1101 in the language: ", dfa.contains(["1","1","0","1"]))


 