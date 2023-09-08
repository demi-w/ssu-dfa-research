totalSkipped = 0
total = 0
totalTime = 0
totalSolve = 0
with open("flexdata.txt") as f:
    for line in f.readlines():
        useful = line.split("|")
        solveTime = float(useful[-2].split()[0][1:])
        totals = useful[3].split()[0].split("/")
        totalSkipped += int(totals[0])
        total += int(totals[1])
        totalTime += int(useful[-1].split()[0])
        totalSolve += solveTime * (int(totals[1]) - int(totals[0]))
print(totalSkipped,"/",total)
print(totalTime / 1000, "seconds")
print(totalSolve / 1000,"overall seconds spent solving boards")
print(totalSolve / total * 1000, "average microseconds spent per board")