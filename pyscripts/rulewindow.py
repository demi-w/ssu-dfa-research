rules = [["110","001"],["011","100"]]
garbage_char = "0"
target_string = "11"
strings = {}
all_strings = set()
new_strings = set()
new_strings.add(target_string)
all_strings.add(target_string)
lhs_len = 3-1
for i in range(8):
    strings = new_strings
    new_strings = set()
    print("iteration:",i,"|",*strings)
    for s in strings:
        for j in range(len(s) + lhs_len):
            l_edge = max(lhs_len-j,0)
            r_edge = min(len(s)-j+1,2)+1
            inspected_sub = s[j+l_edge-lhs_len:j+1]
            inspected_sub = garbage_char*l_edge + inspected_sub + garbage_char * (lhs_len-r_edge+1)
            #print(j,l_edge,r_edge,inspected_sub)
            
            for rule in rules:
                #print(rule[1][l_edge:r_edge])
                if rule[1] == inspected_sub:
                    new_string = s[:j+l_edge-lhs_len] + rule[0] + s[j+1:]
                    new_string = new_string.strip(garbage_char)
                    if new_string not in all_strings and new_string[::-1] not in all_strings: #assumption for symmmetrical dfas
                        new_strings.add(new_string)
                        all_strings.add(new_string)

                    #print(rule[1],rule[1][l_edge:r_edge],new_string)
        