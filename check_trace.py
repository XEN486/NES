def check_traces(path1, path2):
    with open(path1, 'r') as f:
        first = f.readlines()
    with open(path2, 'r') as f:
        second = f.readlines()

    for i, line in enumerate(first):
        if first[i][:73] != second[i][:73]:
            print(f'MISMATCH @ L{i}!\nEXPECTED: {second[i][:73]}\nGOT:      {first[i][:73]}')
            break

check_traces('cpu.log', 'nestest.log')
