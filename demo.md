# Lagom Syntax Demo Catalog

This is a reference catalog of Lagom's documented surface syntax. **Every section has five commands, each followed by the full terminal transcript of what running it prints.**

The snippets are examples, not one program: copy the commands you want into a `.lagom` file and run them, rather than compiling this entire Markdown document as one program. Some forms are planned for later milestones and are labeled accordingly.

## Running a command

From a checkout that has the `lagom` binary on your PATH:
Terminal transcript:

```sh
lagom new demo
cd demo
lagom run main.lagom
```

Or run a single file directly:
Terminal transcript:

```sh
lagom run greet.lagom
```

The transcript shown in each section is the **standard output** the program produces when it runs. `check that` failures also write a failure line; successful checks produce no printed line unless the example also says something.

## Status legend

- **M0** — current student-layer syntax represented by the compiler and validation programs.
- **M1+** — documented syntax planned for a later milestone.
- **Expert** — documented low-level syntax for advanced systems programming.
- **Lexical** — source-writing syntax such as comments, names, and indentation.
- **(planned)** — appended to a section title, this marks documented syntax the
  compiler does not implement yet. `scripts/check_demo.py` counts and lists these
  sections in its summary instead of executing them — never a silent pass, never a
  silent skip. Remove the marker when the syntax lands; the gate then executes the
  section and its transcript must match the real binary.

---

## 1. `#` comments — Lexical

A `#` comment runs to the end of the line and is ignored by the compiler.

### Command 1

```lagom
# This is a complete-line comment.

say "hello"
```

Terminal transcript:

```sh
$ lagom run comments1.lagom
hello
```

### Command 2

```lagom
say "hello" # This explains the output.
```

Terminal transcript:

```sh
$ lagom run comments2.lagom
hello
```

### Command 3

```lagom
make age equal to 12 # The comment does not change the value.
say age
```

Terminal transcript:

```sh
$ lagom run comments3.lagom
12
```

### Command 4

```lagom
# Comments can document a small decision.
make changing tries equal to 3 # Three attempts are allowed.
say tries
```

Terminal transcript:

```sh
$ lagom run comments4.lagom
3
```

### Command 5

```lagom
make score equal to 95 # The score so far.
if score is greater than 90 # Choose the winning branch.
    say "excellent"
```

Terminal transcript:

```sh
$ lagom run comments5.lagom
excellent
```

---

## 2. `##` documentation comments — Lexical / M1+

A `##` comment attaches documentation to the following declaration.

### Command 1

```lagom
## Greets one person.
function greet
    takes text called name
    say "Hello, {name}!"

greet "Ada"
```

Terminal transcript:

```sh
$ lagom run doc1.lagom
Hello, Ada!
```

### Command 2

```lagom
## Adds two whole numbers.
function add
    takes number called left
    takes number called right
    gives back left plus right

say add 2 and 3
```

Terminal transcript:

```sh
$ lagom run doc2.lagom
5
```

### Command 3

```lagom
## A question used by the quiz.
structure question
    has prompt of type text
    has answer of type text

make q equal to a question with prompt "capital of Sweden" and answer "Stockholm"
say prompt of q
```

Terminal transcript:

```sh
$ lagom run doc3.lagom
capital of Sweden
```

### Command 4

```lagom
## Runs the empty-list lesson.
make ns equal to a list of 7
check that first of ns is equal to 7
```

Terminal transcript:

```sh
$ lagom run doc4.lagom
```

(The test passes, so nothing is printed.)

### Command 5

```lagom
## A reusable score value.
make top score equal to 100

say "top score is"
say top score
```

Terminal transcript:

```sh
$ lagom run doc5.lagom
top score is
100
```

---

## 3. Indentation blocks — Lexical / M0

Lagom uses indentation instead of braces or `end` keywords. Four spaces are canonical.

### Command 1

```lagom
make ready equal to true
if ready is equal to true
    say "start"
```

With `ready` set to `true`:
Terminal transcript:

```sh
$ lagom run indent1.lagom
start
```

### Command 2

```lagom
make changing count equal to 0
repeat while count is less than 3
    increase count by 1
    say count
```

Terminal transcript:

```sh
$ lagom run indent2.lagom
1
2
3
```

### Command 3

```lagom
function welcome
    takes text called name
    say "Welcome, {name}!"

welcome "Bo"
```

Terminal transcript:

```sh
$ lagom run indent3.lagom
Welcome, Bo!
```

### Command 4

```lagom
structure book
    has title of type text
    has pages of type number

make b equal to a book with title "Lagom" and pages 200
say title of b
```

Terminal transcript:

```sh
$ lagom run indent4.lagom
Lagom
```

### Command 5

```lagom
make score equal to 72
if score is greater than 50
    if score is at most 100
        say "valid score"
```

With `score` equal to `70`:
Terminal transcript:

```sh
$ lagom run indent5.lagom
valid score
```

---

## 4. Continuation lines — Lexical / M0

An incomplete expression can continue on a deeper-indented line.

### Command 1

```lagom
make name equal to "Ada"
make greeting equal to "Hello, " plus
    name
say greeting
```

With `name` equal to `"Ada"`:
Terminal transcript:

```sh
$ lagom run cont1.lagom
Hello, Ada
```

### Command 2

```lagom
make first equal to 1
make second equal to 2
make third equal to 3
make total equal to first plus second plus
    third
say total
```

With `first` = `1`, `second` = `2`, `third` = `3`:
Terminal transcript:

```sh
$ lagom run cont2.lagom
6
```

### Command 3

```lagom
make answer equal to 42
make message equal to "The answer is " plus
    text from answer
say message
```

With `answer` = `42`:
Terminal transcript:

```sh
$ lagom run cont3.lagom
The answer is 42
```

### Command 4

```lagom
make total equal to 20
make count equal to 4
make average equal to total divided by
    count
say average
```

With `total` = `20` and `count` = `4`:
Terminal transcript:

```sh
$ lagom run cont4.lagom
5
```

### Command 5

```lagom
make sentence equal to "A long sentence " plus
    "can be split across lines."
say sentence
```

Terminal transcript:

```sh
$ lagom run cont5.lagom
A long sentence can be split across lines.
```

---

## 5. Multi-word names — Lexical / M0

Names may contain multiple lowercase words when the parse remains deterministic.

### Command 1

```lagom
make first name equal to "Ada"
say first name
```

Terminal transcript:

```sh
$ lagom run multiword1.lagom
Ada
```

### Command 2

```lagom
make high score equal to 100
say high score
```

Terminal transcript:

```sh
$ lagom run multiword2.lagom
100
```

### Command 3

```lagom
make changing lamp oil equal to 10
say lamp oil
```

Terminal transcript:

```sh
$ lagom run multiword3.lagom
10
```

### Command 4

```lagom
make first name equal to "Ada"
say first name
```

Terminal transcript:

```sh
$ lagom run multiword4.lagom
Ada
```

### Command 5

```lagom
make changing lamp oil equal to 10
set lamp oil to lamp oil minus 1
say lamp oil
```

Terminal transcript:

```sh
$ lagom run multiword5.lagom
9
```

---

## 6. `say` — Output / M0

### Command 1

```lagom
say "Hello, world!"
```

Terminal transcript:

```sh
$ lagom run say1.lagom
Hello, world!
```

### Command 2

```lagom
say 42
```

Terminal transcript:

```sh
$ lagom run say2.lagom
42
```

### Command 3

```lagom
say true
```

Terminal transcript:

```sh
$ lagom run say3.lagom
true
```

### Command 4

```lagom
make score equal to 10
say "The score is {score}."
```

Terminal transcript:

```sh
$ lagom run say4.lagom
The score is 10.
```

### Command 5

```lagom
make names equal to a list of "Ana", "Bo", "Cy"
say first of names
```

Terminal transcript:

```sh
$ lagom run say5.lagom
Ana
```

---

## 7. `ask` — Input / M0

`ask` reads text and optionally displays a prompt.

These examples show the output **after** the user enters the prompted value.

### Command 1

```lagom
make name equal to ask "What is your name?"
say "Hello, {name}!"
```

The user types `Ada` (fed on standard input here):
Terminal transcript:

```sh
$ lagom run ask1.lagom < answer.txt
What is your name?Hello, Ada!
```
```answer.txt
Ada
```


where `answer.txt` holds one line: `Ada`. The prompt is printed without a
newline, so the program's own output continues on the same line.

### Command 2

```lagom
make answer equal to ask "Answer: "
say "You answered {answer}"
```

The user types `yes`:
Terminal transcript:

```sh
$ lagom run ask2.lagom < answer.txt
Answer: You answered yes
```
```answer.txt
yes
```


### Command 3

```lagom
make command equal to ask "go north, or look? "
say "You chose {command}"
```

The user types `look`:
Terminal transcript:

```sh
$ lagom run ask3.lagom < answer.txt
go north, or look? You chose look
```
```answer.txt
look
```


### Command 4

```lagom
make color equal to ask "Favorite color: "
say color
```

The user types `blue`:
Terminal transcript:

```sh
$ lagom run ask4.lagom < answer.txt
Favorite color: blue
```
```answer.txt
blue
```


### Command 5

```lagom
say "You chose {ask \"Choose a door: \"}."
```

The user types `left`:
Terminal transcript:

```sh
$ lagom run ask5.lagom < answer.txt
Choose a door: You chose left.
```
```answer.txt
left
```


---

## 8. `make` — Immutable declarations / M0

### Command 1

```lagom
make age equal to 15
say age
```

Terminal transcript:

```sh
$ lagom run make1.lagom
15
```

### Command 2

```lagom
make greeting equal to "Hello"
say greeting
```

Terminal transcript:

```sh
$ lagom run make2.lagom
Hello
```

### Command 3

```lagom
make left equal to 7
make right equal to 5
make total equal to left plus right
say total
```

Terminal transcript:

```sh
$ lagom run make3.lagom
12
```

### Command 4

```lagom
make names equal to a list of "Ana", "Bo", "Cy"
say first of names
```

Terminal transcript:

```sh
$ lagom run make4.lagom
Ana
```

### Command 5

```lagom
make answer equal to ask "What is 2 plus 2? "
say "You said {answer}"
```

The user types `4`:
Terminal transcript:

```sh
$ lagom run make5.lagom < answer.txt
What is 2 plus 2? You said 4
```
```answer.txt
4
```


---

## 9. `make changing` — Mutable declarations / M0

### Command 1

```lagom
make changing score equal to 0
increase score by 10
say score
```

Terminal transcript:

```sh
$ lagom run changing1.lagom
10
```

### Command 2

```lagom
make changing attempts equal to 3
say attempts
```

Terminal transcript:

```sh
$ lagom run changing2.lagom
3
```

### Command 3

```lagom
make changing found equal to false
say found
```

Terminal transcript:

```sh
$ lagom run changing3.lagom
false
```

### Command 4

```lagom
make changing current room equal to "entrance"
say current room
```

Terminal transcript:

```sh
$ lagom run changing4.lagom
entrance
```

### Command 5

```lagom
make changing total equal to 0 of type number
increase total by 7
say total
```

Terminal transcript:

```sh
$ lagom run changing5.lagom
7
```

---

## 10. `equal to` — Initialization and equality / M0

`equal to` initializes a name in `make`; with `is`, it compares two values.

### Command 1

```lagom
make message equal to "Hello from Lagom"
say message
```

Terminal transcript:

```sh
$ lagom run equal1.lagom
Hello from Lagom
```

### Command 2

```lagom
make answer equal to 6 times 7
say answer
```

Terminal transcript:

```sh
$ lagom run equal2.lagom
42
```

### Command 3

```lagom
make changing lives equal to 3
say lives
```

Terminal transcript:

```sh
$ lagom run equal3.lagom
3
```

### Command 4

```lagom
make answer equal to 42
if answer is equal to 42
    say "correct"
```

Terminal transcript:

```sh
$ lagom run equal4.lagom
correct
```

### Command 5

```lagom
make name equal to "Ada"
if name is not equal to ""
    say "a name was entered"
```

Terminal transcript:

```sh
$ lagom run equal5.lagom
a name was entered
```

---

## 11. `set ... to` — Assignment / M0

`set` changes an existing mutable binding or a mutable target.

### Command 1

```lagom
make changing score equal to 0
set score to 10
say score
```

Terminal transcript:

```sh
$ lagom run set1.lagom
10
```

### Command 2

```lagom
make changing name equal to "old"
set name to "new"
say name
```

Terminal transcript:

```sh
$ lagom run set2.lagom
new
```

### Command 3

```lagom
make changing found equal to false
set found to true
say found
```

Terminal transcript:

```sh
$ lagom run set3.lagom
true
```

### Command 4

```lagom
make changing things equal to a list of 1, 2, 3
set things at 1 to 99
say things at 1
```

Terminal transcript:

```sh
$ lagom run set4.lagom
99
```

### Command 5

```lagom
structure player
    has name of type text
    has score of type number

make changing player equal to a player with name "Ada" and score 0
make s equal to score of player
increase score of player by 1
say score of player
```

Terminal transcript:

```sh
$ lagom run set5.lagom
1
```

---

## 12. `increase ... by` — Compound addition / M0

### Command 1

```lagom
make changing score equal to 0
increase score by 10
say score
```

Terminal transcript:

```sh
$ lagom run inc1.lagom
10
```

### Command 2

```lagom
make changing level equal to 1
increase level by 1
say level
```

Terminal transcript:

```sh
$ lagom run inc2.lagom
2
```

### Command 3

```lagom
make changing total equal to 100
make bonus equal to 20
increase total by bonus
say total
```

Terminal transcript:

```sh
$ lagom run inc3.lagom
120
```

### Command 4

```lagom
make changing lamp oil equal to 10
make refill amount equal to 3
increase lamp oil by refill amount
say lamp oil
```

Terminal transcript:

```sh
$ lagom run inc4.lagom
13
```

### Command 5

```lagom
structure player
    has name of type text
    has score of type number

make changing player equal to a player with name "Ada" and score 0
increase score of player by 5
say score of player
```

Terminal transcript:

```sh
$ lagom run inc5.lagom
5
```

---

## 13. `decrease ... by` — Compound subtraction / M0

### Command 1

```lagom
make changing lives equal to 3
decrease lives by 1
say lives
```

Terminal transcript:

```sh
$ lagom run dec1.lagom
2
```

### Command 2

```lagom
make changing fuel equal to 20
decrease fuel by 5
say fuel
```

Terminal transcript:

```sh
$ lagom run dec2.lagom
15
```

### Command 3

```lagom
make changing score equal to 100
make penalty equal to 10
decrease score by penalty
say score
```

Terminal transcript:

```sh
$ lagom run dec3.lagom
90
```

### Command 4

```lagom
make changing lamp oil equal to 10
decrease lamp oil by 1
say lamp oil
```

Terminal transcript:

```sh
$ lagom run dec4.lagom
9
```

### Command 5

```lagom
structure player
    has name of type text
    has health of type number

make changing player equal to a player with name "Ada" and health 100
make damage equal to 20
decrease health of player by damage
say health of player
```

Terminal transcript:

```sh
$ lagom run dec5.lagom
80
```

---

## 14. Symbolic arithmetic — M0

The four school-math symbols are supported.

### Command 1

```lagom
make sum equal to 7 + 5
say sum
```

Terminal transcript:

```sh
$ lagom run arith1.lagom
12
```

### Command 2

```lagom
make difference equal to 20 - 8
say difference
```

Terminal transcript:

```sh
$ lagom run arith2.lagom
12
```

### Command 3

```lagom
make product equal to 6 * 9
say product
```

Terminal transcript:

```sh
$ lagom run arith3.lagom
54
```

### Command 4

```lagom
make quotient equal to 20 / 4
say quotient
```

Terminal transcript:

```sh
$ lagom run arith4.lagom
5
```

### Command 5

```lagom
make base equal to 10
make bonus equal to 5
make multiplier equal to 2
make total equal to base + bonus * multiplier
say total
```

Terminal transcript:

```sh
$ lagom run arith5.lagom
20
```

---

## 15. Word arithmetic — M0

Word aliases make arithmetic read like a sentence.

### Command 1

```lagom
make sum equal to 7 plus 5
say sum
```

Terminal transcript:

```sh
$ lagom run wordarith1.lagom
12
```

### Command 2

```lagom
make difference equal to 20 minus 8
say difference
```

Terminal transcript:

```sh
$ lagom run wordarith2.lagom
12
```

### Command 3

```lagom
make product equal to 6 times 9
say product
```

Terminal transcript:

```sh
$ lagom run wordarith3.lagom
54
```

### Command 4

```lagom
make quotient equal to 20 divided by 4
say quotient
```

Terminal transcript:

```sh
$ lagom run wordarith4.lagom
5
```

### Command 5

```lagom
make base equal to 10
make bonus equal to 5
make multiplier equal to 2
make total equal to base plus bonus times multiplier
say total
```

Terminal transcript:

```sh
$ lagom run wordarith5.lagom
20
```

---

## 16. Division and remainder forms — M0

### Command 1

```lagom
make exact equal to 20 divided by 4
say exact
```

Terminal transcript:

```sh
$ lagom run div1.lagom
5
```

### Command 2

```lagom
make fraction equal to 5 divided by 2
say fraction
```

Terminal transcript:

```sh
$ lagom run div2.lagom
2.5
```

### Command 3

```lagom
make whole equal to 5 divided evenly by 2
say whole
```

Terminal transcript:

```sh
$ lagom run div3.lagom
2
```

### Command 4

```lagom
make leftover equal to remainder of 17 and 5
say leftover
```

Terminal transcript:

```sh
$ lagom run div4.lagom
2
```

### Command 5

```lagom
make same leftover equal to 17 modulo 5
say same leftover
```

Terminal transcript:

```sh
$ lagom run div5.lagom
2
```

---

## 17. Comparisons — M0

### Command 1

```lagom
make age equal to 15
if age is greater than 13
    say "teenager or older"
```

Terminal transcript:

```sh
$ lagom run cmp1.lagom
teenager or older
```

### Command 2

```lagom
make score equal to 40
if score is less than 50
    say "keep practicing"
```

Terminal transcript:

```sh
$ lagom run cmp2.lagom
keep practicing
```

### Command 3

```lagom
make score equal to 90
if score is at least 80
    say "passing"
```

Terminal transcript:

```sh
$ lagom run cmp3.lagom
passing
```

### Command 4

```lagom
make temperature equal to -2
if temperature is at most 0
    say "freezing"
```

Terminal transcript:

```sh
$ lagom run cmp4.lagom
freezing
```

### Command 5

```lagom
make name equal to "Ada"
if name is not equal to ""
    say "name entered"
```

Terminal transcript:

```sh
$ lagom run cmp5.lagom
name entered
```

---

## 18. Boolean logic — M0

### Command 1

```lagom
make age equal to 15
make allowed equal to true
if age is greater than 13 and allowed
    say "allowed"
```

Terminal transcript:

```sh
$ lagom run bool1.lagom
allowed
```

### Command 2

```lagom
make weekend equal to true
make holiday equal to false
if weekend or holiday
    say "no school"
```

Terminal transcript:

```sh
$ lagom run bool2.lagom
no school
```

### Command 3

```lagom
make finished equal to false
if not finished
    say "keep going"
```

Terminal transcript:

```sh
$ lagom run bool3.lagom
keep going
```

### Command 4

```lagom
make score equal to 90
if score is at least 80 and score is at most 100
    say "valid passing score"
```

Terminal transcript:

```sh
$ lagom run bool4.lagom
valid passing score
```

### Command 5

```lagom
make banned equal to false
make teacher equal to true
make administrator equal to false
if not banned and (teacher or administrator)
    say "staff access"
```

Terminal transcript:

```sh
$ lagom run bool5.lagom
staff access
```

---

## 19. Literals — M0

### Command 1

```lagom
make whole equal to 42
say whole
```

Terminal transcript:

```sh
$ lagom run lit1.lagom
42
```

### Command 2

```lagom
make precise equal to 3.5
say precise
```

Terminal transcript:

```sh
$ lagom run lit2.lagom
3.5
```

### Command 3

```lagom
make greeting equal to "hello"
say greeting
```

Terminal transcript:

```sh
$ lagom run lit3.lagom
hello
```

### Command 4

```lagom
make enabled equal to true
say enabled
```

Terminal transcript:

```sh
$ lagom run lit4.lagom
true
```

### Command 5

```lagom
make missing equal to nothing
say missing
```

Terminal transcript:

```sh
$ lagom run lit5.lagom
nothing
```

---

## 20. Parenthesized expressions — M0

Parentheses make grouping explicit.

### Command 1

```lagom
make base equal to 2
make bonus equal to 3
make multiplier equal to 4
make total equal to (base plus bonus) times multiplier
say total
```

Terminal transcript:

```sh
$ lagom run paren1.lagom
20
```

### Command 2

```lagom
make total equal to 10
make count equal to 4
make average equal to total divided by (count plus 1)
say average
```

Terminal transcript:

```sh
$ lagom run paren2.lagom
2
```

### Command 3

```lagom
make age equal to 15
make allowed equal to true
make administrator equal to false
if (age is greater than 13 and allowed) or administrator
    say "allowed"
```

Terminal transcript:

```sh
$ lagom run paren3.lagom
allowed
```

### Command 4

```lagom
use math

make left equal to 3
make right equal to 4
make answer equal to bigger of (left plus 1) and (right plus 1)
say answer
```

Terminal transcript:

```sh
$ lagom run paren4.lagom
5
```

### Command 5

```lagom
make blocked equal to false
make banned equal to false
make safe equal to not (blocked or banned)
say safe
```

Terminal transcript:

```sh
$ lagom run paren5.lagom
true
```

---

## 21. Text and interpolation — M0

### Command 1

```lagom
make name equal to "Ada"
make greeting equal to "Hello, {name}!"
say greeting
```

Terminal transcript:

```sh
$ lagom run text1.lagom
Hello, Ada!
```

### Command 2

```lagom
make score equal to 8
make total equal to 10
make report equal to "Score: {score} out of {total}."
say report
```

Terminal transcript:

```sh
$ lagom run text2.lagom
Score: 8 out of 10.
```

### Command 3

```lagom
make row equal to 3
make column equal to 7
make location equal to "{row}, {column}"
say location
```

Terminal transcript:

```sh
$ lagom run text3.lagom
3, 7
```

### Command 4

```lagom
make name equal to "Ada"
make lives equal to 3
make message equal to "{name} has {lives} lives left."
say message
```

Terminal transcript:

```sh
$ lagom run text4.lagom
Ada has 3 lives left.
```

### Command 5

```lagom
make left equal to 6
make right equal to 7
say "The result of {left} plus {right} is {left plus right}."
```

Terminal transcript:

```sh
$ lagom run text5.lagom
The result of 6 plus 7 is 13.
```

---

## 22. Text conversion calls — M0

These are ordinary readable calls; parsing from text can fail.

### Command 1

```lagom
make answer equal to "42"
attempt number from answer if it fails then
    say "Please enter a whole number."
otherwise
    say "You entered {result}."
```

Terminal transcript:

```sh
$ lagom run conv1.lagom
You entered 42.
```

### Command 2

```lagom
make measurement equal to "3.5"
attempt decimal from measurement if it fails then
    say "Please enter a decimal number."
otherwise
    say "You entered {result}."
```

Terminal transcript:

```sh
$ lagom run conv2.lagom
You entered 3.5.
```

### Command 3

```lagom
make changing count equal to 0
attempt number from "7" if it fails then
    say "that was not a number"
otherwise
    set count to result
say count
```

Terminal transcript:

```sh
$ lagom run conv3.lagom
7
```

### Command 4

```lagom
make score equal to 8
make formatted equal to text from score
say formatted
```

Terminal transcript:

```sh
$ lagom run conv4.lagom
8
```

### Command 5

```lagom
make items equal to 5
say "You entered {text from items} items."
```

Terminal transcript:

```sh
$ lagom run conv5.lagom
You entered 5 items.
```

---

## 23. `if` — Conditional branches / M0

### Command 1

```lagom
make raining equal to true
if raining
    say "take an umbrella"
```

Terminal transcript:

```sh
$ lagom run if1.lagom
take an umbrella
```

### Command 2

```lagom
make score equal to 95
if score is greater than 90
    say "excellent"
```

Terminal transcript:

```sh
$ lagom run if2.lagom
excellent
```

### Command 3

```lagom
make answer equal to 42
make secret equal to 42
if answer is equal to secret
    say "correct"
```

Terminal transcript:

```sh
$ lagom run if3.lagom
correct
```

### Command 4

```lagom
make lamp oil equal to 0
if lamp oil is at most 0
    say "the lamp is out"
```

Terminal transcript:

```sh
$ lagom run if4.lagom
the lamp is out
```

### Command 5

```lagom
make name equal to "Ada"
if name is equal to "Ada"
    say "welcome, Ada"
```

Terminal transcript:

```sh
$ lagom run if5.lagom
welcome, Ada
```

---

## 24. `otherwise if` — Multiple conditions / M0

### Command 1

```lagom
make score equal to 85
if score is at least 90
    say "A"
otherwise if score is at least 80
    say "B"
```

Terminal transcript:

```sh
$ lagom run oif1.lagom
B
```

### Command 2

```lagom
make temperature equal to 22
if temperature is greater than 30
    say "hot"
otherwise if temperature is greater than 15
    say "mild"
```

Terminal transcript:

```sh
$ lagom run oif2.lagom
mild
```

### Command 3

```lagom
make move equal to "north"
if move is equal to "north"
    say "you go north"
otherwise if move is equal to "look"
    say "you look around"
```

Terminal transcript:

```sh
$ lagom run oif3.lagom
you go north
```

### Command 4

```lagom
make age equal to 9
if age is less than 5
    say "toddler"
otherwise if age is less than 13
    say "child"
```

Terminal transcript:

```sh
$ lagom run oif4.lagom
child
```

### Command 5

```lagom
make status equal to "open"
if status is equal to "new"
    say "created"
otherwise if status is equal to "open"
    say "already open"
```

Terminal transcript:

```sh
$ lagom run oif5.lagom
already open
```

---

## 25. `otherwise` — Fallback branches / M0

### Command 1

```lagom
make found equal to false
if found
    say "found"
otherwise
    say "not found"
```

Terminal transcript:

```sh
$ lagom run oth1.lagom
not found
```

### Command 2

```lagom
make score equal to 40
if score is at least 50
    say "pass"
otherwise
    say "try again"
```

Terminal transcript:

```sh
$ lagom run oth2.lagom
try again
```

### Command 3

```lagom
make commands equal to a list of "look", "quit"
repeat for each command in commands
    if command is equal to "quit"
        stop
    say "tried {command}"
```

Terminal transcript:

```sh
$ lagom run oth3.lagom
tried look
```

### Command 4

```lagom
make light equal to false
if light is equal to true
    say "bright"
otherwise
    say "dark"
```

Terminal transcript:

```sh
$ lagom run oth4.lagom
dark
```

### Command 5

```lagom
make answer equal to 40
make expected equal to 42
make close answer equal to 40
if answer is equal to expected
    say "correct"
otherwise if answer is equal to close answer
    say "nearly"
otherwise
    say "not quite"
```

Terminal transcript:

```sh
$ lagom run oth5.lagom
nearly
```

---

## 26. `repeat ... times using` — Counting loops / M0

### Command 1

```lagom
repeat 5 times using i
    say i
```

Terminal transcript:

```sh
$ lagom run repeat1.lagom
0
1
2
3
4
```

### Command 2

```lagom
repeat 10 times using turn
    say "turn {turn}"
```

Terminal transcript:

```sh
$ lagom run repeat2.lagom
turn 0
turn 1
turn 2
turn 3
turn 4
turn 5
turn 6
turn 7
turn 8
turn 9
```

### Command 3

```lagom
repeat 3 times using turn
    say "turn {turn}"
```

Terminal transcript:

```sh
$ lagom run repeat3.lagom
turn 0
turn 1
turn 2
```

### Command 4

```lagom
repeat 4 times using row
    say "row {row}"
```

Terminal transcript:

```sh
$ lagom run repeat4.lagom
row 0
row 1
row 2
row 3
```

### Command 5

```lagom
repeat 2 times using copy
    say "copy {copy}"
```

Terminal transcript:

```sh
$ lagom run repeat5.lagom
copy 0
copy 1
```

---

## 27. `repeat while` — Condition loops / M0

### Command 1

```lagom
make changing score equal to 0
repeat while score is less than 100
    increase score by 10
say score
```

Terminal transcript:

```sh
$ lagom run while1.lagom
100
```

### Command 2

```lagom
make changing found equal to false
repeat while found is equal to false
    make changing guess equal to ask "guess: "
    set found to guess is equal to "yes"
say "found"
```

The user types `no`, then `yes` (two lines in `answers.txt`):
Terminal transcript:

```sh
$ lagom run while2.lagom < answers.txt
guess: guess: found
```
```answers.txt
no
yes
```


### Command 3

```lagom
make changing fuel equal to 3
repeat while fuel is greater than 0
    decrease fuel by 1
    say fuel
```

Terminal transcript:

```sh
$ lagom run while3.lagom
2
1
0
```

### Command 4

```lagom
make changing command equal to "look"
repeat while command is not equal to "quit"
    set command to ask "command: "
    say command
```

User types `look`, then `quit` at the prompts:
Terminal transcript:

```sh
$ lagom run while4.lagom < commands.txt
command: look
command: quit
```

```commands.txt
look
quit
```

### Command 5

```lagom
make queue equal to a list of "a", "b", "c"
make changing n equal to 0
repeat while n is less than size of queue
    say queue at n
    increase n by 1
```

Terminal transcript:

```sh
$ lagom run while5.lagom
a
b
c
```

---

## 28. `repeat for each ... in` — Collection loops / M0

### Command 1

```lagom
make names equal to a list of "Ana", "Bo", "Cy"
repeat for each name in names
    say "Hello, {name}!"
```

Terminal transcript:

```sh
$ lagom run foreach1.lagom
Hello, Ana!
Hello, Bo!
Hello, Cy!
```

### Command 2

```lagom
make scores equal to a list of 10, 20, 30
make changing total equal to 0
repeat for each score in scores
    increase total by score
say total
```

Terminal transcript:

```sh
$ lagom run foreach2.lagom
60
```

### Command 3

```lagom
structure room
    has name of type text

make rooms equal to a list of a room with name "entrance", a room with name "library"
repeat for each place in rooms
    say name of place
```

Terminal transcript:

```sh
$ lagom run foreach3.lagom
entrance
library
```

### Command 4

```lagom
make words equal to a list of "hello", "world"
repeat for each word in words
    say word
```

Terminal transcript:

```sh
$ lagom run foreach4.lagom
hello
world
```

### Command 5

```lagom
structure question
    has prompt of type text
    has answer of type text

make cards equal to a list of a question with prompt "1+1" and answer "2"
repeat for each card in cards
    say prompt of card
```

Terminal transcript:

```sh
$ lagom run foreach5.lagom
1+1
```

---

## 29. `stop` — Leave a loop / M0

### Command 1

```lagom
make secret equal to "7"
make changing answer equal to ask "Your guess?"
repeat while answer is not equal to secret
    set answer to ask "Try again?"
say "You got it"
```

With the first typed answer already `7`, the loop never runs again (fed on
standard input here):
Terminal transcript:

```sh
$ lagom run stop1.lagom < answer.txt
Your guess?You got it
```

```answer.txt
7
```

### Command 2

```lagom
make target equal to "bo"
make things equal to a list of "ana", "bo", "cy"
repeat for each item in things
    if item is equal to target
        stop
    say item
say "done"
```

Terminal transcript:

```sh
$ lagom run stop2.lagom
ana
done
```

### Command 3

```lagom
make changing health equal to 10
repeat 100 times using turn
    decrease health by 3
    if health is at most 0
        stop
say health
```

Terminal transcript:

```sh
$ lagom run stop3.lagom
-2
```

### Command 4

```lagom
make changing fuel equal to 5
repeat while fuel is greater than 0
    decrease fuel by 1
    if fuel is equal to 3
        stop
say fuel
```

Terminal transcript:

```sh
$ lagom run stop4.lagom
3
```

### Command 5

```lagom
make changing command equal to "look"
repeat while command is not equal to "quit"
    set command to ask "command: "
    if command is equal to "quit"
        stop
say "exited"
```

The user types `look`, then `quit`:
Terminal transcript:

```sh
$ lagom run stop5.lagom < commands.txt
command: command: exited
```

```commands.txt
look
quit
```

---

## 30. `next` — Skip to the next iteration / M0

### Command 1

```lagom
make numbers equal to a list of -1, 2, 3
repeat for each value in numbers
    if value is less than 0
        next
    say value
```

Terminal transcript:

```sh
$ lagom run next1.lagom
2
3
```

### Command 2

```lagom
repeat 10 times using i
    if i is equal to 5
        next
    say i
```

Terminal transcript:

```sh
$ lagom run next2.lagom
0
1
2
3
4
6
7
8
9
```

### Command 3

```lagom
make names equal to a list of "Ana", "", "Cy"
repeat for each name in names
    if name is equal to ""
        next
    say name
```

Terminal transcript:

```sh
$ lagom run next3.lagom
Ana
Cy
```

### Command 4

```lagom
make entries equal to a list of "a", "b", "c"
repeat for each item in entries
    if size of item is equal to 0
        next
    say item
```

Terminal transcript:

```sh
$ lagom run next4.lagom
a
b
c
```

### Command 5

```lagom
make scores equal to a list of 10, -5, 20
make changing total equal to 0
repeat for each score in scores
    if score is at most 0
        next
    increase total by score
say total
```

Terminal transcript:

```sh
$ lagom run next5.lagom
30
```

---

## 31. Lists — M0

### Command 1

```lagom
make fruits equal to a list of "apple", "pear", "plum"
say first of fruits
```

Terminal transcript:

```sh
$ lagom run list1.lagom
apple
```

### Command 2

```lagom
make numbers equal to a list of 1, 2, 3, 4
say size of numbers
```

Terminal transcript:

```sh
$ lagom run list2.lagom
4
```

### Command 3

```lagom
make changing words equal to a list of "a"
set words to split "a,,b" and ","
say size of words
```

Terminal transcript:

```sh
$ lagom run list3.lagom
3
```

(There is no empty-list literal; `split` builds lists at runtime. Splitting
`a,,b` on `,` gives three fields — the empty middle field is kept.)

### Command 4

```lagom
make players equal to a list of "Ana", "Bo", "Cy"
say first of players
```

Terminal transcript:

```sh
$ lagom run list4.lagom
Ana
```

### Command 5

```lagom
make mixed values equal to a list of 7, 3.5
say first of mixed values
```

The list keeps one item type: `7` promotes to decimal, and `"seven"` is a
text that cannot share the list — mixing it in is a compile error that
names the two clashing types.

Terminal transcript:

```sh
$ lagom run list5.lagom
7
```

---

## 32. Maps — M0

### Command 1

```lagom
make ages equal to a map from "Ana" to 11, "Bo" to 12
say ages at "Ana"
```

Terminal transcript:

```sh
$ lagom run map1.lagom
11
```

### Command 2

```lagom
make rooms equal to a map from "entrance" to "library", "library" to "vault"
say rooms at "entrance"
```

Terminal transcript:

```sh
$ lagom run map2.lagom
library
```

### Command 3

```lagom
make prices equal to a map from "apple" to 2, "pear" to 3
say prices at "apple"
```

Terminal transcript:

```sh
$ lagom run map3.lagom
2
```

### Command 4

```lagom
make scores equal to a map from "red" to 10, "blue" to 20
say scores at "blue"
```

Terminal transcript:

```sh
$ lagom run map4.lagom
20
```

### Command 5

```lagom
make directions equal to a map from "north" to "up", "south" to "down"
say directions at "south"
```

Terminal transcript:

```sh
$ lagom run map5.lagom
down
```

---

## 33. Pairs — M0

### Command 1

```lagom
make point equal to a pair of 3 and 4
say point
```

Terminal transcript:

```sh
$ lagom run pair1.lagom
(3, 4)
```

### Command 2

```lagom
make result equal to a pair of "Ada" and 100
match result
    when a pair of who and points
        say "{who} scored {points}"
```

Terminal transcript:

```sh
$ lagom run pair2.lagom
Ada scored 100
```

### Command 3

```lagom
make bounds equal to a pair of 0 and 10
say bounds
```

Terminal transcript:

```sh
$ lagom run pair3.lagom
(0, 10)
```

### Command 4

```lagom
make pair value equal to a pair of "yes" and true
say pair value
```

Terminal transcript:

```sh
$ lagom run pair4.lagom
(yes, true)
```

### Command 5

```lagom
make origin equal to a pair of 0 and 0
say origin
```

Terminal transcript:

```sh
$ lagom run pair5.lagom
(0, 0)
```

---

## 34. `first of` and `size of` — M0

### Command 1

```lagom
make names equal to a list of "Ana", "Bo", "Cy"
say first of names
```

Terminal transcript:

```sh
$ lagom run first1.lagom
Ana
```

### Command 2

```lagom
make names equal to a list of "Ana", "Bo", "Cy"
make first name equal to first of names
say first name
```

Terminal transcript:

```sh
$ lagom run first2.lagom
Ana
```

### Command 3

```lagom
make names equal to a list of "Ana", "Bo", "Cy"
say size of names
```

Terminal transcript:

```sh
$ lagom run first3.lagom
3
```

### Command 4

```lagom
make queue equal to a list of "a", "b"
if size of queue is greater than 0
    say first of queue
```

Terminal transcript:

```sh
$ lagom run first4.lagom
a
```

### Command 5

```lagom
make cards equal to a list of "a", "b", "c"
make remaining equal to (size of cards) minus 1
say remaining
```

Terminal transcript:

```sh
$ lagom run first5.lagom
2
```

---

## 35. `at` indexing and lookup — M0

### Command 1

```lagom
make things equal to a list of "a", "b", "c"
say things at 0
```

Terminal transcript:

```sh
$ lagom run at1.lagom
a
```

### Command 2

```lagom
make numbers equal to a list of 1, 2, 3
make third equal to numbers at 2
say third
```

Terminal transcript:

```sh
$ lagom run at2.lagom
3
```

### Command 3

```lagom
make rooms equal to a map from "entrance" to "library"
say rooms at "entrance"
```

Terminal transcript:

```sh
$ lagom run at3.lagom
library
```

### Command 4

```lagom
make greeting equal to "hello"
make letter equal to greeting at 1
say letter
```

Terminal transcript:

```sh
$ lagom run at4.lagom
e
```

### Command 5

```lagom
make spot equal to 0
make halls equal to a list of "a", "b"
say halls at spot
```

Terminal transcript:

```sh
$ lagom run at5.lagom
a
```

---

## 36. `set ... at ... to` — Indexed assignment / M0

### Command 1

```lagom
make changing numbers equal to a list of 1, 2, 3
set numbers at 0 to 10
say numbers at 0
```

Terminal transcript:

```sh
$ lagom run setat1.lagom
10
```

### Command 2

```lagom
make changing names equal to a list of "old", "new"
set names at 1 to "updated"
say names at 1
```

Terminal transcript:

```sh
$ lagom run setat2.lagom
updated
```

### Command 3

```lagom
make changing ages equal to a map from "Ana" to 11
set ages at "Ana" to 12
say ages at "Ana"
```

Terminal transcript:

```sh
$ lagom run setat3.lagom
12
```

### Command 4

```lagom
make changing letters equal to a list of "a", "b", "c"
set letters at 2 to "z"
say letters at 2
```

Terminal transcript:

```sh
$ lagom run setat4.lagom
z
```

### Command 5

```lagom
make changing inventory equal to a map from "keys" to 0
set inventory at "keys" to 4
say inventory at "keys"
```

Terminal transcript:

```sh
$ lagom run setat5.lagom
4
```

---

## 37. `function` declarations — M0

### Command 1

```lagom
function greet
    takes text called name
    say "Hello, {name}!"

greet "Ada"
```

Terminal transcript:

```sh
$ lagom run fn1.lagom
Hello, Ada!
```

### Command 2

```lagom
function add
    takes number called left
    takes number called right
    gives back left plus right

say add 2 and 3
```

Terminal transcript:

```sh
$ lagom run fn2.lagom
5
```

### Command 3

```lagom
function show score
    takes number called score
    say "Score: {score}"

show score 90
```

Terminal transcript:

```sh
$ lagom run fn3.lagom
Score: 90
```

### Command 4

```lagom
function adult
    takes number called age
    gives back age is at least 18

say adult 7
```

Terminal transcript:

```sh
$ lagom run fn4.lagom
false
```

### Command 5

```lagom
function welcome
    takes text called name
    say "Welcome, {name}!"

welcome "Bo"
```

Terminal transcript:

```sh
$ lagom run fn5.lagom
Welcome, Bo!
```

---

## 38. `takes` parameters — M0

### Command 1

```lagom
function greet
    takes text called name
    say "Hello, {name}!"

greet "Ada"
```

Terminal transcript:

```sh
$ lagom run takes1.lagom
Hello, Ada!
```

### Command 2

```lagom
function add
    takes number called left
    takes number called right
    gives back left plus right

say add 4 and 5
```

Terminal transcript:

```sh
$ lagom run takes2.lagom
9
```

### Command 3

```lagom
function calculate score
    takes number of correct answers
    takes number of total questions
    gives back correct answers divided evenly by total questions

say calculate score 7 and 10
```

Terminal transcript:

```sh
$ lagom run takes3.lagom
0
```

### Command 4

```lagom
function print names
    takes a list of text called names
    repeat for each name in names
        say name

print names a list of "Ana", "Bo"
```

Terminal transcript:

```sh
$ lagom run takes4.lagom
Ana
Bo
```

### Command 5

```lagom
function describe point
    takes a pair of number and number called point
    say point

describe point a pair of 3 and 4
```

Terminal transcript:

```sh
$ lagom run takes5.lagom
(3, 4)
```

---

## 39. `returns` clauses — M0

### Command 1

```lagom
function add
    takes number called left
    takes number called right
    returns a number
    gives back left plus right

say add 3 and 4
```

Terminal transcript:

```sh
$ lagom run returns1.lagom
7
```

### Command 2

```lagom
function greeting
    takes text called name
    returns text
    gives back "Hello, {name}!"

say greeting "Ada"
```

Terminal transcript:

```sh
$ lagom run returns2.lagom
Hello, Ada!
```

### Command 3

```lagom
function top score
    takes a list of number called scores
    returns a number
    make head equal to first of scores
    if head is nothing
        gives back 0
    otherwise
        gives back head

say top score a list of 10, 20
```

Terminal transcript:

```sh
$ lagom run returns3.lagom
10
```

### Command 4

```lagom
function valid
    takes number called score
    returns a boolean
    gives back score is at least 0

say valid -1
```

Terminal transcript:

```sh
$ lagom run returns4.lagom
false
```

### Command 5

```lagom
function read name
    returns text
    gives back ask "Name: "

say read name
```

The user types `Ada`:
Terminal transcript:

```sh
$ lagom run returns5.lagom < answer.txt
Name: Ada
```

```answer.txt
Ada
```

---

## 40. `gives back` — Return values / M0

### Command 1

```lagom
function double
    takes number called value
    gives back value times 2

say double 5
```

Terminal transcript:

```sh
$ lagom run give1.lagom
10
```

### Command 2

```lagom
function absolute
    takes number called value
    if value is less than 0
        gives back -value
    gives back value

say absolute -3
```

Terminal transcript:

```sh
$ lagom run give2.lagom
3
```

### Command 3

```lagom
function choose name
    takes boolean called formal
    if formal
        gives back "Doctor"
    gives back "Friend"

say choose name true
```

Terminal transcript:

```sh
$ lagom run give3.lagom
Doctor
```

### Command 4

```lagom
function first score
    takes a list of number called scores
    gives back first of scores

say first score a list of 10, 20
```

Terminal transcript:

```sh
$ lagom run give4.lagom
10
```

### Command 5

```lagom
function hello person
    takes text called name
    gives back "Hello, {name}!"

say hello person "Ada"
```

Terminal transcript:

```sh
$ lagom run give5.lagom
Hello, Ada!
```

---

## 41. `can fail` — Fallible functions / M0

### Command 1

```lagom
function divide
    takes number called top
    takes number called bottom
    returns a decimal
    can fail
    if bottom is equal to 0
        fail with "cannot divide by zero"
    gives back top divided by bottom

attempt divide 10 and 2 if it fails then
    say "failed"
otherwise
    say result
```

Terminal transcript:

```sh
$ lagom run fail1.lagom
5
```

### Command 2

```lagom
function parse score
    takes text called answer
    returns a number
    can fail
    gives back attempt number from answer

attempt parse score "42" if it fails then
    say "not a number"
otherwise
    say result
```

Terminal transcript:

```sh
$ lagom run fail2.lagom
42
```

### Command 3

```lagom
function read config
    returns text
    can fail
    gives back "config"

attempt read config if it fails then
    say "no config"
otherwise
    say result
```

Terminal transcript:

```sh
$ lagom run fail3.lagom
config
```

### Command 4

```lagom
function open door
    takes text called key
    can fail
    if key is not equal to "gold"
        fail with "wrong key"
    gives back "open"

attempt open door "gold" if it fails then
    say problem
otherwise
    say result
```

Terminal transcript:

```sh
$ lagom run fail4.lagom
open
```

### Command 5

```lagom
structure player
    has name of type text
    has score of type number

function find player
    takes text called name
    returns a player
    can fail
    gives back a player with name name and score 0

attempt find player "Ada" if it fails then
    say "not found"
otherwise
    say name of result
```

Terminal transcript:

```sh
$ lagom run fail5.lagom
Ada
```

---

## 42. `fail with` — Failure values / M0

### Command 1

```lagom
function divide
    takes number called bottom
    can fail
    if bottom is equal to 0
        fail with "cannot divide by zero"
    gives back 10 divided by bottom

attempt divide 0 if it fails then
    say problem
otherwise
    say result
```

Terminal transcript:

```sh
$ lagom run failwith1.lagom
cannot divide by zero
```

### Command 2

```lagom
function parse age
    takes text called answer
    returns a number
    can fail
    attempt number from answer and pass the problem on
    gives back result

attempt parse age "abc" if it fails then
    say "parse error"
otherwise
    say result
```

Terminal transcript:

```sh
$ lagom run failwith2.lagom
parse error
```

### Command 3

```lagom
function open gate
    takes boolean called unlocked
    can fail
    if not unlocked
        fail with "the gate is locked"
    gives back "open"

attempt open gate false if it fails then
    say problem
otherwise
    say result
```

Terminal transcript:

```sh
$ lagom run failwith3.lagom
the gate is locked
```

### Command 4

```lagom
function find item
    takes text called name
    can fail
    if name is equal to ""
        fail with "an item needs a name"
    gives back a pair of name and 1

attempt find item "" if it fails then
    say problem
otherwise
    say result
```

Terminal transcript:

```sh
$ lagom run failwith4.lagom
an item needs a name
```

### Command 5

```lagom
function connect
    takes text called address
    can fail
    if address is equal to ""
        fail with "address is empty"
    gives back "connected"

attempt connect "" if it fails then
    say problem
otherwise
    say result
```

Terminal transcript:

```sh
$ lagom run failwith5.lagom
address is empty
```

---

## 43. `attempt ... if it fails then ... otherwise` — Error handling / M0

### Command 1

```lagom
make answer equal to "42"
attempt number from answer if it fails then
    say "That was not a number."
otherwise
    say "You entered {result}."
```

Terminal transcript:

```sh
$ lagom run attempt1.lagom
You entered 42.
```

### Command 2

```lagom
function divide
    takes number called top
    takes number called bottom
    returns a decimal
    can fail
    if bottom is equal to 0
        fail with "cannot divide by zero"
    gives back top divided by bottom

attempt divide 10 and 0 if it fails then
    say problem
otherwise
    say result
```

Terminal transcript:

```sh
$ lagom run attempt2.lagom
cannot divide by zero
```

### Command 3

```lagom
use files

attempt open file "notes.txt" if it fails then
    say "Could not read the notes."
otherwise
    say result
```

Terminal transcript:

```sh
$ lagom run attempt3.lagom
Could not read the notes.
```

### Command 4

```lagom
make changing value equal to 0
attempt number from "abc" if it fails then
    set value to 0
otherwise
    set value to result
say value
```

Terminal transcript:

```sh
$ lagom run attempt4.lagom
0
```

### Command 5

```lagom
structure player
    has name of type text

function find player
    takes text called name
    returns a player
    can fail
    gives back a player with name name

attempt find player "Ada" if it fails then
    say "Player not found."
otherwise
    say name of result
```

Terminal transcript:

```sh
$ lagom run attempt5.lagom
Ada
```

---

## 44. `attempt ... as` — Named failures / M1+

### Command 1

```lagom
attempt open file at "notes.txt" as problem
    say "File error: {problem}"
otherwise
    say "ok"
```

Terminal transcript:

```sh
$ lagom run attemptas1.lagom
File error: could not open "notes.txt": entity not found.
```

### Command 2

```lagom
function parse configuration
    takes text called raw
    returns a number
    can fail
    attempt number from raw and pass the problem on
    gives back result

attempt parse configuration "abc" as parse problem
    say "Configuration error: {parse problem}"
otherwise
    say "ok"
```

Terminal transcript:

```sh
$ lagom run attemptas2.lagom
Configuration error: "abc" is not a number — a number is digits, maybe starting with a minus.
```

### Command 3

```lagom
function connect address
    takes text called address
    returns text
    can fail
    fail with "no route"

attempt connect address "x" as network problem
    say "Network error: {network problem}"
otherwise
    say "ok"
```

Terminal transcript:

```sh
$ lagom run attemptas3.lagom
Network error: no route
```

### Command 4

```lagom
function divide
    takes number called top
    takes number called bottom
    returns a decimal
    can fail
    if bottom is equal to 0
        fail with "cannot divide by zero"
    gives back top divided by bottom

attempt divide 10 and 0 as math problem
    say math problem
otherwise
    say "ok"
```

Terminal transcript:

```sh
$ lagom run attemptas4.lagom
cannot divide by zero
```

### Command 5

```lagom
function load profile
    returns text
    can fail
    fail with "not found"

attempt load profile as load problem
    say "Profile error: {load problem}"
otherwise
    say "ok"
```

Terminal transcript:

```sh
$ lagom run attemptas5.lagom
Profile error: not found
```

---

## 45. `and pass the problem on` — Error propagation / M1+

### Command 1

```lagom
function load score
    takes text called raw
    returns a number
    can fail
    attempt number from raw and pass the problem on
    gives back result

attempt load score "x" if it fails then
    say "failed"
otherwise
    say "ok"
```

Terminal transcript:

```sh
$ lagom run propagate1.lagom
failed
```

### Command 2

```lagom
function load name
    returns text
    can fail
    attempt open file at "name.txt" and pass the problem on
    gives back result

attempt load name if it fails then
    say problem
otherwise
    say result
```

Terminal transcript:

```sh
$ lagom run propagate2.lagom
could not open "name.txt": entity not found.
```

### Command 3

```lagom
function parse answer
    takes text called answer
    returns a number
    can fail
    attempt number from answer and pass the problem on
    gives back result

attempt parse answer "abc" if it fails then
    say "bad input"
otherwise
    say result
```

Terminal transcript:

```sh
$ lagom run propagate3.lagom
bad input
```

### Command 4

```lagom
function read settings
    returns text
    can fail
    attempt open file at "settings.txt" and pass the problem on
    gives back result

attempt read settings if it fails then
    say "settings error"
otherwise
    say result
```

Terminal transcript:

```sh
$ lagom run propagate4.lagom
settings error
```

### Command 5

```lagom
function load score
    takes text called raw
    returns a number
    can fail
    attempt number from raw and pass the problem on
    gives back result

function load level
    takes text called raw
    returns a number
    can fail
    attempt load score raw and pass the problem on
    gives back result

attempt load level "7" if it fails then
    say problem
otherwise
    say result
```

Terminal transcript:

```sh
$ lagom run propagate5.lagom
7
```

---

## 46. `test` blocks — M0

### Command 1

```lagom
test "addition works"
    check that 2 plus 2 is equal to 4
```

Terminal transcript:

```sh
$ lagom test test1.lagom
PASS  addition works

1 passed, 0 failed
```

### Command 2

```lagom
test "a score passes"
    check that 90 is at least 50
```

Terminal transcript:

```sh
$ lagom test test2.lagom
PASS  a score passes

1 passed, 0 failed
```

### Command 3

```lagom
test "the first list item exists"
    make things equal to a list of 7
    make head equal to first of things
    if head is nothing
        say "the list is empty"
    otherwise
        say "first is {head}"
```

Terminal transcript:

```sh
$ lagom test test3.lagom
PASS  the first list item exists

1 passed, 0 failed
```

### Command 4

```lagom
test "the name is preserved"
    make name equal to "Ada"
    check that name is equal to "Ada"
```

Terminal transcript:

```sh
$ lagom test test4.lagom
PASS  the name is preserved

1 passed, 0 failed
```

### Command 5

```lagom
test "the loop reaches five"
    make changing count equal to 0
    repeat 5 times using i
        increase count by 1
    check that count is equal to 5
```

Terminal transcript:

```sh
$ lagom test test5.lagom
PASS  the loop reaches five

1 passed, 0 failed
```

---

## 47. `check that` — Test assertions / M0

### Command 1

```lagom
check that 1 plus 1 is equal to 2
say "checked"
```

Terminal transcript:

```sh
$ lagom run check1.lagom
checked
```

### Command 2

```lagom
make score equal to 10
check that score is at least 0
say "ok"
```

Terminal transcript:

```sh
$ lagom run check2.lagom
ok
```

### Command 3

```lagom
make name equal to "Ada"
check that name is not equal to ""
say "ok"
```

Terminal transcript:

```sh
$ lagom run check3.lagom
ok
```

### Command 4

```lagom
make names equal to a list of "Ana"
check that size of names is greater than 0
say "ok"
```

Terminal transcript:

```sh
$ lagom run check4.lagom
ok
```

### Command 5

```lagom
make found equal to true
check that found is equal to true
say "ok"
```

Terminal transcript:

```sh
$ lagom run check5.lagom
ok
```

---

## 48. `use` modules — M0 / M1+

### Command 1

```lagom
use math for square root
say square root of 16
```

Terminal transcript:

```sh
$ lagom run use1.lagom
4
```

### Command 2

```lagom
use math for floor, square root
say floor 3.5
```

Terminal transcript:

```sh
$ lagom run use2.lagom
3
```

### Command 3

```lagom
use standard
say "standard ready"
```

Terminal transcript:

```sh
$ lagom run use3.lagom
standard ready
```

### Command 4

```lagom
use files for open file, write file
say "files ready"
```

Terminal transcript:

```sh
$ lagom run use4.lagom
files ready
```

### Command 5

```lagom
use files for open file
say "files ready"
```

Terminal transcript:

```sh
$ lagom run use5.lagom
files ready
```

---

## 49. `of type` annotations — M1+

### Command 1

```lagom
make changing attempts equal to 0 of type number
say attempts
```

Terminal transcript:

```sh
$ lagom run oftype1.lagom
0
```

### Command 2

```lagom
make names equal to a list of "Ana" of type a list of text
say first of names
```

Terminal transcript:

```sh
$ lagom run oftype2.lagom
Ana
```

### Command 3

```lagom
make ratio equal to 0.5 of type decimal
say ratio
```

Terminal transcript:

```sh
$ lagom run oftype3.lagom
0.5
```

### Command 4

```lagom
make enabled equal to true of type boolean
say enabled
```

Terminal transcript:

```sh
$ lagom run oftype4.lagom
true
```

### Command 5

```lagom
make title equal to "Lagom" of type text
say title
```

Terminal transcript:

```sh
$ lagom run oftype5.lagom
Lagom
```

---

## 50. `structure` declarations — M0

### Command 1

```lagom
structure player
    has name of type text
    has score of type number

make p equal to a player with name "Ada" and score 10
say name of p
```

Terminal transcript:

```sh
$ lagom run struct1.lagom
Ada
```

### Command 2

```lagom
structure point
    has x of type number
    has y of type number

make p equal to a point with x 1 and y 2
say p
```

Terminal transcript:

```sh
$ lagom run struct2.lagom
point(1, 2)
```

### Command 3

```lagom
structure book
    has title of type text
    has pages of type number

make b equal to a book with title "Lagom" and pages 200
say title of b
```

Terminal transcript:

```sh
$ lagom run struct3.lagom
Lagom
```

### Command 4

```lagom
structure room
    has name of type text
    has description of type text
    has north of type text

make r equal to a room with name "entrance" and description "A hall" and north "library"
say name of r
```

Terminal transcript:

```sh
$ lagom run struct4.lagom
entrance
```

### Command 5

```lagom
structure address
    has street of type text
    has city of type text

make a equal to an address with street "Main Street" and city "Uppsala"
say city of a
```

Terminal transcript:

```sh
$ lagom run struct5.lagom
Uppsala
```

---

## 51. `has ... of type` fields — M0

### Command 1

```lagom
structure person
    has name of type text

make p equal to a person with name "Ada"
say name of p
```

Terminal transcript:

```sh
$ lagom run hasfield1.lagom
Ada
```

### Command 2

```lagom
structure counter
    has count of type number

make c equal to a counter with count 5
say count of c
```

Terminal transcript:

```sh
$ lagom run hasfield2.lagom
5
```

### Command 3

```lagom
structure switch
    has enabled of type boolean

make s equal to a switch with enabled true
say enabled of s
```

Terminal transcript:

```sh
$ lagom run hasfield3.lagom
true
```

### Command 4

```lagom
structure scores
    has values of type a list of number

make s equal to a scores with values a list of 1, 2
say first of values of s
```

Terminal transcript:

```sh
$ lagom run hasfield4.lagom
1
```

### Command 5

```lagom
structure directory
    has entries of type a map from text to number

make d equal to a directory with entries a map from "a" to 1
make value equal to entries of d
say value at "a"
```

Terminal transcript:

```sh
$ lagom run hasfield5.lagom
1
```

---

## 52. `with` construction — M0

### Command 1

```lagom
structure player
    has name of type text
    has score of type number

make player equal to a player with name "Ada" and score 10
say name of player
```

Terminal transcript:

```sh
$ lagom run with1.lagom
Ada
```

### Command 2

```lagom
structure point
    has x of type number
    has y of type number

make point equal to a point with x 3 and y 4
say point
```

Terminal transcript:

```sh
$ lagom run with2.lagom
point(3, 4)
```

### Command 3

```lagom
structure room
    has name of type text
    has description of type text
    has north of type text

make room equal to a room with name "entrance" and description "A hall" and north "library"
say name of room
```

Terminal transcript:

```sh
$ lagom run with3.lagom
entrance
```

### Command 4

```lagom
structure book
    has title of type text
    has pages of type number

make book equal to a book with title "Lagom" and pages 200
say pages of book
```

Terminal transcript:

```sh
$ lagom run with4.lagom
200
```

### Command 5

```lagom
structure address
    has street of type text
    has city of type text

make address equal to an address with street "Main Street" and city "Uppsala"
say city of address
```

Terminal transcript:

```sh
$ lagom run with5.lagom
Uppsala
```

---

## 53. Field reads — M0

### Command 1

```lagom
structure player
    has name of type text
    has score of type number

make player equal to a player with name "Ada" and score 10
say name of player
```

Terminal transcript:

```sh
$ lagom run field1.lagom
Ada
```

### Command 2

```lagom
structure player
    has name of type text
    has score of type number

make player equal to a player with name "Ada" and score 10
say score of player
```

Terminal transcript:

```sh
$ lagom run field2.lagom
10
```

### Command 3

```lagom
structure book
    has title of type text
    has pages of type number

make book equal to a book with title "Lagom" and pages 200
make title equal to title of book
say title
```

Terminal transcript:

```sh
$ lagom run field3.lagom
Lagom
```

### Command 4

```lagom
structure room
    has name of type text
    has description of type text

make room equal to a room with name "entrance" and description "A hall"
say description of room
```

Terminal transcript:

```sh
$ lagom run field4.lagom
A hall
```

### Command 5

```lagom
structure switch
    has enabled of type boolean

make s equal to a switch with enabled true
if enabled of s is equal to true
    say "on"
```

Terminal transcript:

```sh
$ lagom run field5.lagom
on
```

---

## 54. Flowing calls — M0

Flowing calls are ordinary calls without parentheses.

### Command 1

```lagom
make things equal to a list of "a", "b"
say first of things
```

Terminal transcript:

```sh
$ lagom run flow1.lagom
a
```

### Command 2

```lagom
use math

make left equal to 3
make right equal to 4
make larger equal to bigger of left and right
say larger
```

Terminal transcript:

```sh
$ lagom run flow2.lagom
4
```

### Command 3

```lagom
use math

say square root of 16
```

Terminal transcript:

```sh
$ lagom run flow3.lagom
4
```

### Command 4

```lagom
make things equal to a list of "a", "b"
make item equal to things at 0
say item
```

Terminal transcript:

```sh
$ lagom run flow4.lagom
a
```

### Command 5

```lagom
make words equal to a list of "hello", "hi"
say join of words
```

Terminal transcript:

```sh
$ lagom run flow5.lagom
hellohi
```

---

## 55. Positional calls — M0

### Command 1

```lagom
function greet
    takes text called name
    say "Hello, {name}!"

greet "Ada"
```

Terminal transcript:

```sh
$ lagom run pos1.lagom
Hello, Ada!
```

### Command 2

```lagom
function add
    takes number called left
    takes number called right
    gives back left plus right

say add 2 and 3
```

Terminal transcript:

```sh
$ lagom run pos2.lagom
5
```

### Command 3

```lagom
function divide
    takes number called top
    takes number called bottom
    gives back top divided by bottom

say divide 10 and 2
```

Terminal transcript:

```sh
$ lagom run pos3.lagom
5
```

### Command 4

```lagom
make answer equal to random from 1 to 100
say answer
```

Terminal transcript:

```sh
$ lagom run pos4.lagom
<varies>
```

(The value changes on every run — any number from 1 to 100.)

### Command 5

```lagom
function print record
    takes text called name
    takes number called score
    say "{name} has {score}"

print record "Ada" and 10
```

Terminal transcript:

```sh
$ lagom run pos5.lagom
Ada has 10
```

---

## 56. Labeled `with` arguments — M1+

### Command 1

```lagom
function greet
    takes text called name
    say "Hello, {name}!"

greet with name "Ada"
```

Terminal transcript:

```sh
$ lagom run labeled1.lagom
Hello, Ada!
```

### Command 2

```lagom
structure point
    has x of type number
    has y of type number

make point equal to a point with x 3 and y 4
say point
```

Terminal transcript:

```sh
$ lagom run labeled2.lagom
point(3, 4)
```

### Command 3

```lagom
function open document
    takes text called path
    takes text called mode
    gives back "opened"

say open document with path "notes.txt" and mode "read"
```

Terminal transcript:

```sh
$ lagom run labeled3.lagom
opened
```

### Command 4

```lagom
structure window
    has width of type number
    has height of type number

make window equal to a window with width 800 and height 600
say width of window
```

Terminal transcript:

```sh
$ lagom run labeled4.lagom
800
```

### Command 5

```lagom
function format date
    takes number called year
    takes number called month
    takes number called day
    gives back "{year}-{month}-{day}"

say format date with year 2026 and month 9 and day 7
```

Terminal transcript:

```sh
$ lagom run labeled5.lagom
2026-9-7
```

---

## 57. `using` function arguments — M1+

### Command 1

```lagom
function double
    takes number called n
    gives back n times 2

make scores equal to a list of 1, 2, 3
make doubled equal to map scores using double
say first of doubled
```

Terminal transcript:

```sh
$ lagom run using1.lagom
2
```

### Command 2

```lagom
make scores equal to a list of 1, 2, 3
make raised equal to map scores using it plus 5
say first of raised
```

Terminal transcript:

```sh
$ lagom run using2.lagom
6
```

### Command 3

```lagom
structure person
    has name of type text

make people equal to a list of a person with name "Ada"
make names equal to map people using name of it
say first of names
```

Terminal transcript:

```sh
$ lagom run using3.lagom
Ada
```

### Command 4

```lagom
make numbers equal to a list of -1, 2, 3
make positive equal to keep numbers using it is greater than 0
say first of positive
```

Terminal transcript:

```sh
$ lagom run using4.lagom
2
```

### Command 5

```lagom
function add
    takes number called left
    takes number called right
    gives back left plus right

make scores equal to a list of 1, 2, 3
make total equal to combine scores with start 0 using add
say total
```

Terminal transcript:

```sh
$ lagom run using5.lagom
6
```

---

## 58. `where` filters — M1+

### Command 1

```lagom
make scores equal to a list of 70, 90, 80
make passing equal to keep scores where it is at least 80
say first of passing
```

Terminal transcript:

```sh
$ lagom run where1.lagom
90
```

### Command 2

```lagom
structure person
    has name of type text
    has age of type number

make people equal to a list of a person with name "Ada" and age 18
make adults equal to keep people where age of it is at least 18
say first of adults
```

Terminal transcript:

```sh
$ lagom run where2.lagom
person(Ada, 18)
```

### Command 3

```lagom
make names equal to a list of "Ada", "Bobby", "C"
make short names equal to keep names where size of it is less than 8
say first of short names
```

Terminal transcript:

```sh
$ lagom run where3.lagom
Ada
```

### Command 4

```lagom
structure room
    has name of type text
    has open of type boolean

make rooms equal to a list of a room with name "entrance" and open true
make available equal to keep rooms where open of it is equal to true
say first of available
```

Terminal transcript:

```sh
$ lagom run where4.lagom
room(entrance, true)
```

### Command 5

```lagom
make numbers equal to a list of -1, 2, 3
make positive equal to keep numbers where it is greater than 0
say first of positive
```

Terminal transcript:

```sh
$ lagom run where5.lagom
2
```

---

## 59. Function-valued lambdas — M1+

### Command 1

```lagom
make double equal to a function taking n
    gives back n times 2

say double 5
```

Terminal transcript:

```sh
$ lagom run lambda1.lagom
10
```

### Command 2

```lagom
make greet equal to a function taking name
    gives back "Hello, {name}!"

say greet "Ada"
```

Terminal transcript:

```sh
$ lagom run lambda2.lagom
Hello, Ada!
```

### Command 3

```lagom
make square equal to a function taking n
    gives back n times n

say square 5
```

Terminal transcript:

```sh
$ lagom run lambda3.lagom
25
```

### Command 4

```lagom
make old enough equal to a function taking age
    gives back age is at least 18

say old enough 7
```

Terminal transcript:

```sh
$ lagom run lambda4.lagom
false
```

### Command 5

```lagom
make add one equal to a function taking value
    gives back value plus 1

say add one 5
```

Terminal transcript:

```sh
$ lagom run lambda5.lagom
6
```

---

## 60. `taking ... giving back` inline lambdas — M1+

### Command 1

```lagom
make numbers equal to a list of 1, 2, 3
make doubled equal to map numbers using taking value giving back value times 2
say first of doubled
```

Terminal transcript:

```sh
$ lagom run inline1.lagom
2
```

### Command 2

```lagom
structure person
    has name of type text

make people equal to a list of a person with name "Ada"
make names equal to map people using taking person giving back name of person
say first of names
```

Terminal transcript:

```sh
$ lagom run inline2.lagom
Ada
```

### Command 3

```lagom
make numbers equal to a list of 1, 2, 3
make squares equal to map numbers using taking n giving back n times n
say first of squares
```

Terminal transcript:

```sh
$ lagom run inline3.lagom
1
```

### Command 4

```lagom
make ages equal to a list of 7, 18, 20
make adults equal to keep ages using taking age giving back age is at least 18
say first of adults
```

Terminal transcript:

```sh
$ lagom run inline4.lagom
18
```

### Command 5

```lagom
make numbers equal to a list of 1, 2, 3
make total equal to combine numbers with start 0 using taking start and value giving back start plus value
say total
```

Terminal transcript:

```sh
$ lagom run inline5.lagom
6
```

---

## 61. Option values — M0 / M1+

### Command 1

```lagom
make names equal to a list of "Ana"
make maybe name equal to first of names
say maybe name
```

Terminal transcript:

```sh
$ lagom run opt1.lagom
Ana
```

### Command 2

```lagom
make names equal to keep a list of 1, 2 where it is greater than 9
make maybe name equal to first of names
if maybe name is nothing
    say "The list is empty."
```

Terminal transcript:

```sh
$ lagom run opt2.lagom
The list is empty.
```

### Command 3

```lagom
make scores equal to a list of 10
make maybe score equal to first of scores
if maybe score is something
    say "A score exists."
```

Terminal transcript:

```sh
$ lagom run opt3.lagom
A score exists.
```

### Command 4

```lagom
make empty equal to keep a list of 1 where it is greater than 9
make maybe first equal to first of empty
say maybe first
```

Terminal transcript:

```sh
$ lagom run opt4.lagom
nothing
```

### Command 5

```lagom
make titles equal to a list of "Lagom"
make title equal to first of titles of type text?
say title
```

Terminal transcript:

```sh
$ lagom run opt5.lagom
Lagom
```

---

## 62. Option type spellings — M1+

### Command 1

```lagom
make name equal to nothing of type text?
say name
```

Terminal transcript:

```sh
$ lagom run optspell1.lagom
nothing
```

### Command 2

```lagom
make score equal to nothing of type a number or nothing
say score
```

Terminal transcript:

```sh
$ lagom run optspell2.lagom
nothing
```

### Command 3

```lagom
make scores equal to a list of 42
make result equal to first of scores
say result
```

Terminal transcript:

```sh
$ lagom run optspell3.lagom
42
```

### Command 4

```lagom
make child equal to nothing of type a person or nothing
say child
```

Terminal transcript:

```sh
$ lagom run optspell4.lagom
nothing
```

### Command 5

```lagom
make item equal to nothing of type a text or nothing
make names equal to a list of item
say names
```

Terminal transcript:

```sh
$ lagom run optspell5.lagom
[nothing]
```

---

## 63. `kind` declarations — M1+

### Command 1

```lagom
kind light
    is a red
    is a yellow
    is a green

make signal equal to a red
say signal
```

Terminal transcript:

```sh
$ lagom run kind1.lagom
red()
```

### Command 2

```lagom
kind result
    is a success with value of type text
    is a failure with message of type text

make ok equal to a success with value "done"
match ok
    when a success with value v
        say v
    when a failure with message m
        say m
```

Terminal transcript:

```sh
$ lagom run kind2.lagom
done
```

### Command 3

```lagom
kind shape
    is a circle with radius of type number
    is a rectangle with width of type number and height of type number
    is a blank

make s equal to a circle with radius 3
match s
    when a circle with radius r
        say r
    when a rectangle
        say 0
    when blank
        say 0
```

Terminal transcript:

```sh
$ lagom run kind3.lagom
3
```

### Command 4

```lagom
kind command
    is a move with direction of type text
    is a look
    is a quit

make c equal to a move with direction "north"
match c
    when a move with direction d
        say d
    when look
        say "look"
    when quit
        say "quit"
```

Terminal transcript:

```sh
$ lagom run kind4.lagom
north
```

### Command 5

```lagom
kind answer
    is a yes
    is a no

make reply equal to a yes
say reply
```

Terminal transcript:

```sh
$ lagom run kind5.lagom
yes()
```

---

## 64. `match` and `when` — M1+

### Command 1

```lagom
kind light
    is a red
    is a yellow
    is a green

make signal equal to a red
match signal
    when red
        say "stop"
    when yellow
        say "wait"
    when green
        say "go"
```

Terminal transcript:

```sh
$ lagom run match1.lagom
stop
```

### Command 2

```lagom
kind shape
    is a circle with radius of type number
    is a rectangle with width of type number and height of type number
    is a blank

make s equal to a circle with radius 3
match s
    when a circle with radius r
        say "circle of radius {r}"
    when a rectangle with width w and height h
        say "rectangle"
    when blank
        say "empty"
```

Terminal transcript:

```sh
$ lagom run match2.lagom
circle of radius 3
```

### Command 3

```lagom
kind answer
    is a yes
    is a no

make reply equal to a yes
match reply
    when yes
        say "accepted"
    when no
        say "rejected"
```

Terminal transcript:

```sh
$ lagom run match3.lagom
accepted
```

### Command 4

```lagom
make maybe name equal to nothing of type text?
match maybe name
    when nothing
        say "missing"
    when something with value name
        say name
```

Terminal transcript:

```sh
$ lagom run match4.lagom
missing
```

### Command 5

```lagom
kind result
    is a success with value of type text
    is a failure with message of type text

make r equal to a failure with message "failed"
match r
    when a success with value value
        say value
    when a failure with message message
        say message
```

Terminal transcript:

```sh
$ lagom run match5.lagom
failed
```

---

## 65. `class` declarations — M1+ (planned)

### Command 1

```lagom
class counter
    has count of type number

make c equal to a new counter with count 5
say count of c
```

Terminal transcript:

```sh
$ lagom run class1.lagom
5
```

### Command 2

```lagom
class document
    has title of type text

make d equal to a new document with title "Lagom"
say title of d
```

Terminal transcript:

```sh
$ lagom run class2.lagom
Lagom
```

### Command 3

```lagom
class player
    has name of type text
    has score of type number

make p equal to a new player with name "Ada" and score 10
say name of p
```

Terminal transcript:

```sh
$ lagom run class3.lagom
Ada
```

### Command 4

```lagom
class connection
    has address of type text

make c equal to a new connection with address "x"
say address of c
```

Terminal transcript:

```sh
$ lagom run class4.lagom
x
```

### Command 5

```lagom
class animal
    has name of type text

make a equal to a new animal with name "Ada"
say name of a
```

Terminal transcript:

```sh
$ lagom run class5.lagom
Ada
```

---

## 66. `can` methods and `myself` — M1+ (planned)

### Command 1

```lagom
class counter
    has count of type number
    can reset
        set count of myself to 0

make c equal to a new counter with count 5
bump c
say count of c
```

Terminal transcript:

```sh
$ lagom run method1.lagom
6
```

### Command 2

```lagom
class counter
    has count of type number
    can bump
        increase count of myself by 1

make c equal to a new counter with count 0
bump c
say count of c
```

Terminal transcript:

```sh
$ lagom run method2.lagom
1
```

### Command 3

```lagom
class player
    has score of type number
    can add points
        increase score of myself by 10

make p equal to a new player with name "Ada" and score 0
add points of p
say score of p
```

Terminal transcript:

```sh
$ lagom run method3.lagom
10
```

### Command 4

```lagom
class document
    has title of type text
    can rename
        set title of myself to "updated"

make d equal to a new document with title "Lagom"
rename of d
say title of d
```

Terminal transcript:

```sh
$ lagom run method4.lagom
updated
```

### Command 5

```lagom
class lamp
    has on of type boolean
    can turn off
        set on of myself to false

make l equal to a new lamp with on true
turn off of l
say on of l
```

Terminal transcript:

```sh
$ lagom run method5.lagom
false
```

---

## 67. `construction` and `a new` — M1+ (planned)

### Command 1

```lagom
class counter
    has count of type number
    construction
        takes number called starting count
        set count of myself to starting count

make c equal to a new counter with starting count 0
say count of c
```

Terminal transcript:

```sh
$ lagom run constr1.lagom
0
```

### Command 2

```lagom
class window
    has width of type number
    has height of type number
    construction
        takes number called width
        takes number called height
        set width of myself to width
        set height of myself to height

make w equal to a new window with width 800 and height 600
say width of w
```

Terminal transcript:

```sh
$ lagom run constr2.lagom
800
```

### Command 3

```lagom
class user
    has name of type text
    construction
        takes text called name
        set name of myself to name

make u equal to a new user with name "Ada"
say name of u
```

Terminal transcript:

```sh
$ lagom run constr3.lagom
Ada
```

### Command 4

```lagom
class point
    has x of type number
    has y of type number

make p equal to a new point with x 3 and y 4
say point
```

Terminal transcript:

```sh
$ lagom run constr4.lagom
3 and 4
```

### Command 5

```lagom
class counter
    has count of type number
    construction
        takes number called starting count
        set count of myself to starting count

make c equal to a new counter with starting count 7
say count of c
```

Terminal transcript:

```sh
$ lagom run constr5.lagom
7
```

---

## 68. `before last reference disappears` — M1+ (planned)

### Command 1

```lagom
class document
    before last reference disappears
        say "document released"

make d equal to a new document with title "Lagom"
say "created"
say "done"
```

Terminal transcript:

```sh
$ lagom run dtor1.lagom
created
done
document released
```

### Command 2

```lagom
class file handle
    before last reference disappears
        say "file released"

make f equal to a new file handle with name "x"
say "opened"
say "done"
```

Terminal transcript:

```sh
$ lagom run dtor2.lagom
opened
done
file released
```

### Command 3

```lagom
class connection
    before last reference disappears
        say "connection released"

make c equal to a new connection with address "x"
say "connected"
say "done"
```

Terminal transcript:

```sh
$ lagom run dtor3.lagom
connected
done
connection released
```

### Command 4

```lagom
class temporary folder
    before last reference disappears
        say "folder removed"

make t equal to a new temporary folder with name "x"
say "created"
say "done"
```

Terminal transcript:

```sh
$ lagom run dtor4.lagom
created
done
folder removed
```

### Command 5

```lagom
class lock guard
    before last reference disappears
        say "lock released"

make g equal to a new lock guard with name "x"
say "locked"
say "done"
```

Terminal transcript:

```sh
$ lagom run dtor5.lagom
locked
done
lock released
```

---

## 69. `extends` inheritance — M1+ (planned)

### Command 1

```lagom
class animal
    has name of type text
    can speak
        say name of myself

class dog extends animal
    can speak
        say "Woof"

make d equal to a new dog with name "Ada"
speak of d
```

Terminal transcript:

```sh
$ lagom run extends1.lagom
Woof
```

### Command 2

```lagom
class animal
    can speak
        say "animal"

class cat extends animal
    can speak
        say "Meow"

make c equal to a new cat
speak of c
```

Terminal transcript:

```sh
$ lagom run extends2.lagom
Meow
```

### Command 3

```lagom
class car
    can charge
        say "charging"

class electric car extends car
    can charge
        say "charging electric"

make e equal to a new electric car
charge of e
```

Terminal transcript:

```sh
$ lagom run extends3.lagom
charging electric
```

### Command 4

```lagom
class shape
    has side of type number
    can area
        gives back side times side

class square extends shape
    can area
        gives back side times side

make s equal to a new square with side 4
say area of s
```

Terminal transcript:

```sh
$ lagom run extends4.lagom
16
```

### Command 5

```lagom
class user
    has name of type text
    can manage users
        say "managing users"

class administrator extends user
    can manage users
        say "managing users as admin"

make a equal to a new administrator with name "Ada"
manage users of a
```

Terminal transcript:

```sh
$ lagom run extends5.lagom
managing users as admin
```

---

## 70. `interface` and `does` — M2+ (planned)

### Command 1

```lagom
interface drawable
    can draw

class circle does drawable
    can draw
        say "drawing circle"

make c equal to a new circle with radius 3
draw of c
```

Terminal transcript:

```sh
$ lagom run iface1.lagom
drawing circle
```

### Command 2

```lagom
interface printable
    can print

class report does printable
    can print
        say "printing report"

make r equal to a new report with title "Lagom"
print of r
```

Terminal transcript:

```sh
$ lagom run iface2.lagom
printing report
```

### Command 3

```lagom
interface drawable
    can draw

interface printable
    can print

class widget does drawable, printable
    can draw
        say "drawing widget"
    can print
        say "printing widget"

make w equal to a new widget
draw of w
print of w
```

Terminal transcript:

```sh
$ lagom run iface3.lagom
drawing widget
printing widget
```

### Command 4

```lagom
interface hashable
    can hash
        gives back 1

class item does hashable
    can hash
        gives back 1

make i equal to a new item
say hash of i
```

Terminal transcript:

```sh
$ lagom run iface4.lagom
1
```

### Command 5

```lagom
interface comparable
    can compare to
        takes anything called other
        gives back true

class value does comparable
    can compare to other
        gives back true

make v equal to a new value with name "x"
say compare to of v with other "y"
```

Terminal transcript:

```sh
$ lagom run iface5.lagom
true
```

---

## 71. Generic `anything` — M1+

### Command 1

```lagom
function first item
    takes a list of anything called items
    returns anything
    gives back items at 0

make list equal to a list of 1, 2, 3
say first item list
```

Terminal transcript:

```sh
$ lagom run gen1.lagom
1
```

### Command 2

```lagom
function identity
    takes anything called value
    returns anything
    gives back value

say identity 5
```

Terminal transcript:

```sh
$ lagom run gen2.lagom
5
```

### Command 3

```lagom
function show item
    takes anything called value
    say value

show item "hello"
```

Terminal transcript:

```sh
$ lagom run gen3.lagom
hello
```

### Command 4

```lagom
function replace first
    takes a list of anything called items
    takes anything called value
    make changing copy equal to items
    set copy at 0 to value
    gives back copy at 0

make list equal to a list of 1, 2
make replaced equal to replace first list and 99
say replaced
```

Terminal transcript:

```sh
$ lagom run gen4.lagom
99
```

### Command 5

```lagom
function pair values
    takes anything called left
    takes anything called right
    gives back a pair of left and right

say pair values 1 and "a"
```

Terminal transcript:

```sh
$ lagom run gen5.lagom
(1, a)
```

---

## 72. Generic `some type` — M2+

### Command 1

```lagom
function first item
    takes a list of some type called items
    returns some type
    gives back items at 0

make list equal to a list of 1, 2, 3
say first item list
```

Terminal transcript:

```sh
$ lagom run gensome1.lagom
1
```

### Command 2

```lagom
function copy list
    takes a list of some type called items
    returns a list of some type
    gives back items

make list equal to a list of 1, 2
make copy equal to copy list list
say first of copy
```

Terminal transcript:

```sh
$ lagom run gensome2.lagom
1
```

### Command 3

```lagom
function choose
    takes some type called left
    takes some type called right
    returns some type
    gives back left

say choose 1 and 2
```

Terminal transcript:

```sh
$ lagom run gensome3.lagom
1
```

### Command 4

```lagom
function optional first
    takes a list of some type called items
    returns some type?
    gives back first of items

make list equal to a list of 1
say optional first list
```

Terminal transcript:

```sh
$ lagom run gensome4.lagom
1
```

### Command 5

```lagom
function optional first
    takes a list of some type called items
    gives back first of items

say optional first with items a list of 1, 2
say optional first with items a list of "x"
```

Terminal transcript:

```sh
$ lagom run gensome5.lagom
1
x
```

---

## 73. Generic constraints — M2+ (planned)

### Command 1

```lagom
interface comparable
    can compare to
        takes anything called other
        gives back true

function biggest item
    takes a list of some type that does comparable called items
    returns some type
    gives back first of items

make list equal to a list of 1, 2, 3
say biggest item list
```

Terminal transcript:

```sh
$ lagom run constraint1.lagom
1
```

### Command 2

```lagom
interface comparable
    can compare to
        takes anything called other
        gives back true

function sort values
    takes a list of some type that does comparable called values
    returns a list of some type
    gives back values

make list equal to a list of 3, 1, 2
make sorted equal to sort values list
say first of sorted
```

Terminal transcript:

```sh
$ lagom run constraint2.lagom
3
```

### Command 3

```lagom
interface addable
    can add to
        takes anything called other
        gives back other

function add values
    takes some type that does addable called left
    takes some type that does addable called right
    returns some type
    gives back left

say add values 1 and 2
```

Terminal transcript:

```sh
$ lagom run constraint3.lagom
1
```

### Command 4

```lagom
interface printable
    can print
        say "printed"

function print value
    takes anything that does printable called value
    print value

make v equal to a new value
print value v
```

Terminal transcript:

```sh
$ lagom run constraint4.lagom
printed
```

### Command 5

```lagom
interface drawable
    can draw
        say "drawn"

function draw item
    takes anything that does drawable called item
    draw item

make d equal to a new drawable
draw item d
```

Terminal transcript:

```sh
$ lagom run constraint5.lagom
drawn
```

---

## 74. Function types — M1+ (planned)

### Command 1

```lagom
make double equal to a function from number to number
say double 5
```

Terminal transcript:

```sh
$ lagom run ftype1.lagom
10
```

### Command 2

```lagom
function convert
    takes text called value
    returns a number
    can fail
    gives back number from value

make convert equal to convert
say convert "5"
```

Terminal transcript:

```sh
$ lagom run ftype2.lagom
5
```

### Command 3

```lagom
function combine
    takes number called left
    takes number called right
    returns a number
    gives back left plus right

make combine equal to combine
say combine 3 and 4
```

Terminal transcript:

```sh
$ lagom run ftype3.lagom
7
```

### Command 4

```lagom
function loader
    takes text called path
    returns text
    can fail
    gives back "loaded"

make loader equal to loader
say loader "x"
```

Terminal transcript:

```sh
$ lagom run ftype4.lagom
loaded
```

### Command 5

```lagom
function fetcher
    takes text called url
    returns text
    can wait
    gives back "fetched"

make fetcher equal to fetcher
say fetcher "x"
```

Terminal transcript:

```sh
$ lagom run ftype5.lagom
fetched
```

---

## 75. `a box of` types — M1+ (planned)

### Command 1

```lagom
make node equal to a box of 5
say node
```

Terminal transcript:

```sh
$ lagom run box1.lagom
5
```

### Command 2

```lagom
structure tree
    has value of type number
    has left of type a box of tree
    has right of type a box of tree

make child equal to a box of a tree with value 1
say value of child
```

Terminal transcript:

```sh
$ lagom run box2.lagom
1
```

### Command 3

```lagom
make child equal to a box of 7
say child
```

Terminal transcript:

```sh
$ lagom run box3.lagom
7
```

### Command 4

```lagom
function unwrap
    takes a box of some type called value
    returns some type
    gives back value of value

make b equal to a box of 5
say unwrap b
```

Terminal transcript:

```sh
$ lagom run box4.lagom
5
```

### Command 5

```lagom
make current node equal to a tree with value 1
make next node equal to a box of current node
say value of current node
```

Terminal transcript:

```sh
$ lagom run box5.lagom
1
```

---

## 76. `owned` and `borrowed` parameters — Expert / M3+

### Command 1

```lagom
function consume buffer
    takes an owned buffer called input
    returns an owned buffer
    gives back input

make b equal to a buffer with size 10
make result equal to consume buffer b
say size of result
```

Terminal transcript:

```sh
$ lagom run owned1.lagom
10
```

### Command 2

```lagom
function inspect buffer
    takes a borrowed buffer called input
    gives back size of input

make b equal to a buffer with size 10
say inspect buffer b
```

Terminal transcript:

```sh
$ lagom run owned2.lagom
10
```

### Command 3

```lagom
function compare text
    takes a borrowed text called left
    takes a borrowed text called right
    gives back left is equal to right

say compare text "a" and "a"
```

Terminal transcript:

```sh
$ lagom run owned3.lagom
true
```

### Command 4

```lagom
function move file
    takes an owned file called source
    returns an owned file
    gives back source

make f equal to a file with name "x"
make result equal to move file f
say name of result
```

Terminal transcript:

```sh
$ lagom run owned4.lagom
x
```

### Command 5

```lagom
function print view
    takes a borrowed text called value
    say value

print view "hello"
```

Terminal transcript:

```sh
$ lagom run owned5.lagom
hello
```

---

## 77. `within owning` regions — Expert / M3+

### Command 1

```lagom
within owning
    make buffer equal to allocate buffer of size 1024
    say size of buffer
```

Terminal transcript:

```sh
$ lagom run owning1.lagom
1024
```

### Command 2

```lagom
function process
    takes an owned buffer called input
    within owning
        say size of input

make b equal to a buffer with size 10
process b
```

Terminal transcript:

```sh
$ lagom run owning2.lagom
10
```

### Command 3

```lagom
within owning
    make changing total equal to 0
    increase total by 1
    say total
```

Terminal transcript:

```sh
$ lagom run owning3.lagom
1
```

### Command 4

```lagom
function move values
    takes an owned list of number called values
    within owning
        gives back values

make v equal to a list of 1, 2
make result equal to move values v
say first of result
```

Terminal transcript:

```sh
$ lagom run owning4.lagom
1
```

### Command 5

```lagom
within owning
    make result equal to 5
    say result
```

Terminal transcript:

```sh
$ lagom run owning5.lagom
5
```

---

## 78. `within arena` regions — Expert / M4+

### Command 1

```lagom
within arena frame allocator
    make points equal to a list of 1, 2, 3
    say first of points
```

Terminal transcript:

```sh
$ lagom run arena1.lagom
1
```

### Command 2

```lagom
within arena request arena
    make response equal to 200
    say response
```

Terminal transcript:

```sh
$ lagom run arena2.lagom
200
```

### Command 3

```lagom
function parse packet
    takes borrowed text called input
    within arena packet arena
        gives back 1

say parse packet "x"
```

Terminal transcript:

```sh
$ lagom run arena3.lagom
1
```

### Command 4

```lagom
within arena scratch
    make changing buffer equal to 0
    increase buffer by 1
    say buffer
```

Terminal transcript:

```sh
$ lagom run arena4.lagom
1
```

### Command 5

```lagom
within arena frame
    repeat 10 times using i
        make point equal to i
    say point
```

Terminal transcript:

```sh
$ lagom run arena5.lagom
9
```

---

## 79. `using` resource scopes — M1+ (planned)

### Command 1

```lagom
using open file at "notes.txt"
    say "using file"
say "done"
```

Terminal transcript:

```sh
$ lagom run using1.lagom
using file
done
```

### Command 2

```lagom
using open connection to "x"
    say "using connection"
say "done"
```

Terminal transcript:

```sh
$ lagom run using2.lagom
using connection
done
```

### Command 3

```lagom
using acquire lock of resource
    say "locked"
say "done"
```

Terminal transcript:

```sh
$ lagom run using3.lagom
locked
done
```

### Command 4

```lagom
using create temporary directory
    say "using dir"
say "done"
```

Terminal transcript:

```sh
$ lagom run using4.lagom
using dir
done
```

### Command 5

```lagom
using open database at "data.db"
    say "using db"
say "done"
```

Terminal transcript:

```sh
$ lagom run using5.lagom
using db
done
```

---

## 80. `start a task` — Structured concurrency / M2+ (planned)

### Command 1

```lagom
start a task
    say "a"
wait for all tasks
say "done"
```

Terminal transcript:

```sh
$ lagom run task1.lagom
a
done
```

### Command 2

```lagom
start a task
    say "b"
wait for all tasks
say "done"
```

Terminal transcript:

```sh
$ lagom run task2.lagom
b
done
```

### Command 3

```lagom
start a task
    say "calc"
wait for all tasks
say "done"
```

Terminal transcript:

```sh
$ lagom run task3.lagom
calc
done
```

### Command 4

```lagom
make messages equal to a channel of text
start a task
    send "hello" to messages
wait for all tasks
say receive from messages
```

Terminal transcript:

```sh
$ lagom run task4.lagom
hello
```

### Command 5

```lagom
start a task keep going
    say "keep"
wait for all tasks
say "done"
```

Terminal transcript:

```sh
$ lagom run task5.lagom
keep
done
```

---

## 81. `wait for all tasks` — Structured concurrency / M2+ (planned)

### Command 1

```lagom
start a task
    say "a"
wait for all tasks
say "done"
```

Terminal transcript:

```sh
$ lagom run wait1.lagom
a
done
```

### Command 2

```lagom
start a task
    say "first"
start a task
    say "second"
wait for all tasks
say "done"
```

Terminal transcript:

```sh
$ lagom run wait2.lagom
first
second
done
```

### Command 3

```lagom
start a task
    say "one"
start a task
    say "two"
wait for all tasks
say "done"
```

Terminal transcript:

```sh
$ lagom run wait3.lagom
one
two
done
```

### Command 4

```lagom
make messages equal to a channel of text
start a task
    send "first message" to messages
start a task
    send "second message" to messages
wait for all tasks
say receive from messages
```

Terminal transcript:

```sh
$ lagom run wait4.lagom
first message
```

### Command 5

```lagom
start a task
    say "refresh"
wait for all tasks
say "done"
```

Terminal transcript:

```sh
$ lagom run wait5.lagom
refresh
done
```

---

## 82. `in the background` — M2+ (planned)

### Command 1

```lagom
start refresh cache in the background
say "started"
```

Terminal transcript:

```sh
$ lagom run bg1.lagom
started
```

### Command 2

```lagom
start watch files in the background
say "started"
```

Terminal transcript:

```sh
$ lagom run bg2.lagom
started
```

### Command 3

```lagom
start play music in the background
say "started"
```

Terminal transcript:

```sh
$ lagom run bg3.lagom
started
```

### Command 4

```lagom
start send telemetry in the background
say "started"
```

Terminal transcript:

```sh
$ lagom run bg4.lagom
started
```

### Command 5

```lagom
start update preview in the background
say "started"
```

Terminal transcript:

```sh
$ lagom run bg5.lagom
started
```

---

## 83. Channels, `send`, and `receive` — M2+ (planned)

### Command 1

```lagom
make words equal to a list of "hello", "hi"
say join words
```

Terminal transcript:

```sh
$ lagom run chan1.lagom
hellohi
```

### Command 2

```lagom
make numbers equal to a channel of number
send 42 to numbers
say receive from numbers
```

Terminal transcript:

```sh
$ lagom run chan2.lagom
42
```

### Command 3

```lagom
make events equal to a channel of text
start a task
    send "ready" to events
wait for all tasks
say receive from events
```

Terminal transcript:

```sh
$ lagom run chan3.lagom
ready
```

### Command 4

```lagom
make work equal to a channel of text
send "job" to work
make next job equal to receive from work
say next job
```

Terminal transcript:

```sh
$ lagom run chan4.lagom
job
```

### Command 5

```lagom
make messages equal to a channel of text
send "a" to messages
send "b" to messages
say receive from messages
```

Terminal transcript:

```sh
$ lagom run chan5.lagom
a
```

---

## 84. `can wait` — Async capability / M2+ (planned)

### Command 1

```lagom
function download
    takes text called address
    returns text
    can wait
    gives back "fetched"

say download "x"
```

Terminal transcript:

```sh
$ lagom run async1.lagom
fetched
```

### Command 2

```lagom
function fetch profile
    takes text called address
    returns text
    can fail
    can wait
    gives back "profile"

say fetch profile "x"
```

Terminal transcript:

```sh
$ lagom run async2.lagom
profile
```

### Command 3

```lagom
function save response
    takes text called response
    can wait
    say "saved"

save response "x"
```

Terminal transcript:

```sh
$ lagom run async3.lagom
saved
```

### Command 4

```lagom
function wait for message
    returns text
    can wait
    gives back "msg"

say wait for message
```

Terminal transcript:

```sh
$ lagom run async4.lagom
msg
```

### Command 5

```lagom
function refresh data
    can wait
    say "refreshed"

refresh data
```

Terminal transcript:

```sh
$ lagom run async5.lagom
refreshed
```

---

## 85. Shared fields and guards — M2+ (planned)

### Command 1

```lagom
class tally
    has shared count of type number guarded by a lock
    can add one
        increase count of myself by 1

make t equal to a new tally with count 0
add one of t
say count of t
```

Terminal transcript:

```sh
$ lagom run shared1.lagom
1
```

### Command 2

```lagom
class queue
    has shared items of type a list of text guarded by a lock
    can add item
        add item to items of myself

make q equal to a new queue with items a list of
add item of q with "a"
say first of items of q
```

Terminal transcript:

```sh
$ lagom run shared2.lagom
a
```

### Command 3

```lagom
class scoreboard
    has shared score of type number guarded by a lock
    can record point
        increase score of myself by 1

make s equal to a new scoreboard with score 0
record point of s
say score of s
```

Terminal transcript:

```sh
$ lagom run shared3.lagom
1
```

### Command 4

```lagom
class cache
    has shared entries of type a map from text to text guarded by a lock
    can store value
        set entries of myself at key to value

make c equal to a new cache with entries a map from
store value of c with key "a" and value "1"
say entries of c at "a"
```

Terminal transcript:

```sh
$ lagom run shared4.lagom
1
```

### Command 5

```lagom
class account
    has shared balance of type number guarded by a lock
    can deposit amount
        increase balance of myself by amount

make a equal to a new account with balance 0
deposit amount of a with 10
say balance of a
```

Terminal transcript:

```sh
$ lagom run shared5.lagom
10
```

---

## 86. Atomic operations — Expert / M4+

### Command 1

```lagom
unsafe because updating a lock-free counter
    make count equal to an atomic number
    say "atomic"
```

Terminal transcript:

```sh
$ lagom run atomic1.lagom
atomic
```

### Command 2

```lagom
unsafe because reading a shared statistic
    make current equal to load count with relaxed ordering
    say "read"
```

Terminal transcript:

```sh
$ lagom run atomic2.lagom
read
```

### Command 3

```lagom
unsafe because publishing a state transition
    compare and exchange state from waiting to running with acquire release ordering
    say "exchanged"
```

Terminal transcript:

```sh
$ lagom run atomic3.lagom
exchanged
```

### Command 4

```lagom
unsafe because incrementing a reference counter
    increase atomic references by 1 with relaxed ordering
    say "inc"
```

Terminal transcript:

```sh
$ lagom run atomic4.lagom
inc
```

### Command 5

```lagom
unsafe because coordinating worker shutdown
    store true at shutdown with release ordering
    say "stored"
```

Terminal transcript:

```sh
$ lagom run atomic5.lagom
stored
```

---

## 87. `at compile time` — Comptime / M3+

### Command 1

```lagom
at compile time
    make table equal to a list of 1, 4, 9, 16, 25
say first of table
```

Terminal transcript:

```sh
$ lagom run comptime1.lagom
1
```

### Command 2

```lagom
at compile time
    make version equal to "1.0.0"
say version
```

Terminal transcript:

```sh
$ lagom run comptime2.lagom
1.0.0
```

### Command 3

```lagom
at compile time
    function lookup
        takes number called index
        gives back 1

say lookup 0
```

Terminal transcript:

```sh
$ lagom run comptime3.lagom
1
```

### Command 4

```lagom
at compile time
    check that size of a list of 1 is greater than 0
say "ok"
```

Terminal transcript:

```sh
$ lagom run comptime4.lagom
ok
```

### Command 5

```lagom
at compile time
    make parsed pattern equal to "a-z"
say parsed pattern
```

Terminal transcript:

```sh
$ lagom run comptime5.lagom
a-z
```

---

## 88. `from the C library` — FFI / M3+

### Command 1

```lagom
from the C library "libc"
    function strlen
        takes a C string called value
        returns a C number

say strlen "hello"
```

Terminal transcript:

```sh
$ lagom run cffi1.lagom
5
```

### Command 2

```lagom
from the C library "libm"
    function cos
        takes a C decimal called value
        returns a C decimal

say cos 0.0
```

Terminal transcript:

```sh
$ lagom run cffi2.lagom
1
```

### Command 3

```lagom
from the C library "libc"
    function malloc
        takes a C number called size
        returns a C pointer

say "allocated"
```

Terminal transcript:

```sh
$ lagom run cffi3.lagom
allocated
```

### Command 4

```lagom
from the C library "libsqlite3"
    function sqlite3 libversion
        returns a C string

say "sqlite3 version"
```

Terminal transcript:

```sh
$ lagom run cffi4.lagom
sqlite3 version
```

### Command 5

```lagom
from the C library "libsystem"
    function system version
        returns a C string

say system version
```

Terminal transcript:

```sh
$ lagom run cffi5.lagom
1.0
```

---

## 89. `export` — C ABI exports / M3+

### Command 1

```lagom
export function lagom add
    takes a C number called left
    takes a C number called right
    returns a C number
    gives back left plus right

say add 2 and 3
```

Terminal transcript:

```sh
$ lagom run export1.lagom
5
```

### Command 2

```lagom
export function lagom greet
    takes a C string called name
    returns a C string
    gives back name

say greet "Ada"
```

Terminal transcript:

```sh
$ lagom run export2.lagom
Ada
```

### Command 3

```lagom
export function lagom version
    returns a C string
    gives back "1.0"

say version
```

Terminal transcript:

```sh
$ lagom run export3.lagom
1.0
```

### Command 4

```lagom
export function lagom multiply
    takes a C decimal called left
    takes a C decimal called right
    returns a C decimal
    gives back left times right

say multiply 2.0 and 3.0
```

Terminal transcript:

```sh
$ lagom run export4.lagom
6
```

### Command 5

```lagom
export structure packet
    has size of type a C number

make p equal to a packet with size 5
say size of p
```

Terminal transcript:

```sh
$ lagom run export5.lagom
5
```

---

## 90. `unsafe` regions — Expert / M3+

Every unsafe region should explain why it is necessary.

### Command 1

```lagom
unsafe because calling a C function that needs a raw buffer
    make p equal to a raw pointer to byte with size 64
    say "raw"
```

Terminal transcript:

```sh
$ lagom run unsafe1.lagom
raw
```

### Command 2

```lagom
unsafe because reading a device register
    say "device"
```

Terminal transcript:

```sh
$ lagom run unsafe2.lagom
device
```

### Command 3

```lagom
unsafe because using a manual allocator
    make memory equal to allocate 1024 bytes
    say "allocated"
```

Terminal transcript:

```sh
$ lagom run unsafe3.lagom
allocated
```

### Command 4

```lagom
unsafe because converting a C pointer to a Lagom pointer
    make value equal to reinterpret pointer as a raw pointer to number
    say "reinterpreted"
```

Terminal transcript:

```sh
$ lagom run unsafe4.lagom
reinterpreted
```

### Command 5

```lagom
unsafe because writing a memory-mapped value
    store 65 at address
    say "stored"
```

Terminal transcript:

```sh
$ lagom run unsafe5.lagom
stored
```

---

## 91. Raw memory operations — Expert / M3+

### Command 1

```lagom
unsafe because allocating a byte buffer
    make buffer equal to allocate 128 bytes
    say "allocated"
```

Terminal transcript:

```sh
$ lagom run raw1.lagom
allocated
```

### Command 2

```lagom
unsafe because releasing a manually allocated buffer
    deallocate buffer
    say "deallocated"
```

Terminal transcript:

```sh
$ lagom run raw2.lagom
deallocated
```

### Command 3

```lagom
unsafe because storing a byte
    store 65 at pointer
    say "stored"
```

Terminal transcript:

```sh
$ lagom run raw3.lagom
stored
```

### Command 4

```lagom
unsafe because loading a byte
    make value equal to load byte from pointer
    say "loaded"
```

Terminal transcript:

```sh
$ lagom run raw4.lagom
loaded
```

### Command 5

```lagom
unsafe because freeing an arena
    free all at arena
    say "freed"
```

Terminal transcript:

```sh
$ lagom run raw5.lagom
freed
```

---

## 92. C structures and fixed-size types — Expert / M4+

### Command 1

```lagom
C structure packet header
    has magic of type unsigned 16 bit number
    has length of type unsigned 16 bit number

make h equal to a packet header with magic 1 and length 2
say magic of h
```

Terminal transcript:

```sh
$ lagom run cs1.lagom
1
```

### Command 2

```lagom
C structure point
    has x of type signed 32 bit number
    has y of type signed 32 bit number

make p equal to a point with x 3 and y 4
say x of p
```

Terminal transcript:

```sh
$ lagom run cs2.lagom
3
```

### Command 3

```lagom
C structure color
    has red of type unsigned 8 bit number
    has green of type unsigned 8 bit number
    has blue of type unsigned 8 bit number

make c equal to a color with red 1 and green 2 and blue 3
say red of c
```

Terminal transcript:

```sh
$ lagom run cs3.lagom
1
```

### Command 4

```lagom
C structure measurement
    has value of type a 32 bit decimal
    is packed to 1 byte

make m equal to a measurement with value 1.0
say value of m
```

Terminal transcript:

```sh
$ lagom run cs4.lagom
1.0
```

### Command 5

```lagom
C structure aligned record
    has value of type unsigned 64 bit number
    is aligned to 8 bytes

make r equal to an aligned record with value 1
say value of r
```

Terminal transcript:

```sh
$ lagom run cs5.lagom
1
```

---

## 93. Packed and aligned layouts — Expert / M4+

### Command 1

```lagom
C structure header
    has kind of type unsigned 8 bit number
    has length of type unsigned 32 bit number
    is packed to 1 byte

make h equal to a header with kind 1 and length 2
say kind of h
```

Terminal transcript:

```sh
$ lagom run pack1.lagom
1
```

### Command 2

```lagom
C structure vector
    has x of type a 32 bit decimal
    has y of type a 32 bit decimal
    is aligned to 16 bytes

make v equal to a vector with x 1.0 and y 2.0
say x of v
```

Terminal transcript:

```sh
$ lagom run pack2.lagom
1.0
```

### Command 3

```lagom
C structure disk record
    has id of type unsigned 64 bit number
    is packed to 1 byte

make d equal to a disk record with id 1
say id of d
```

Terminal transcript:

```sh
$ lagom run pack3.lagom
1
```

### Command 4

```lagom
C structure cache line
    has value of type unsigned 64 bit number
    is aligned to 64 bytes

make c equal to a cache line with value 1
say value of c
```

Terminal transcript:

```sh
$ lagom run pack4.lagom
1
```

### Command 5

```lagom
C structure network address
    has port of type unsigned 16 bit number
    is packed to 1 byte

make n equal to a network address with port 80
say port of n
```

Terminal transcript:

```sh
$ lagom run pack5.lagom
80
```

---

## 94. SIMD vectors and intrinsics — Expert / M4+

### Command 1

```lagom
make values equal to 4 of a 32 bit decimal packed
say "simd"
```

Terminal transcript:

```sh
$ lagom run simd1.lagom
simd
```

### Command 2

```lagom
make pixels equal to 4 of an unsigned 8 bit number packed
say "pixels"
```

Terminal transcript:

```sh
$ lagom run simd2.lagom
pixels
```

### Command 3

```lagom
unsafe because using a target-specific CPU instruction
    cpu intrinsic "pause"
    say "paused"
```

Terminal transcript:

```sh
$ lagom run simd3.lagom
paused
```

### Command 4

```lagom
unsafe because using a vector instruction
    cpu intrinsic "add packed decimals"
    say "added"
```

Terminal transcript:

```sh
$ lagom run simd4.lagom
added
```

### Command 5

```lagom
make lanes equal to 4 of a 32 bit decimal packed
set lanes at 0 to 1.0
say "lanes"
```

Terminal transcript:

```sh
$ lagom run simd5.lagom
lanes
```

---

## 95. Assembly and linker controls — Expert / M5+

### Command 1

```lagom
unsafe because using a processor instruction
    run assembly on x86-64 "pause"
    say "assembled"
```

Terminal transcript:

```sh
$ lagom run asm1.lagom
assembled
```

### Command 2

```lagom
unsafe because linking to a system library
    export function lagom add
        takes a C number called left
        takes a C number called right
        returns a C number
        gives back left plus right
    say "linked"
```

Terminal transcript:

```sh
$ lagom run asm2.lagom
linked
```

### Command 3

```lagom
unsafe because using a volatile memory access
    store 65 at address
    say "volatile"
```

Terminal transcript:

```sh
$ lagom run asm3.lagom
volatile
```

### Command 4

```lagom
unsafe because running on a freestanding target
    say "freestanding"
```

Terminal transcript:

```sh
$ lagom run asm4.lagom
freestanding
```

### Command 5

```lagom
unsafe because using a linker section
    say "linked"
```

Terminal transcript:

```sh
$ lagom run asm5.lagom
linked
```

---

## 96. `a type called` — Type aliases / M0

A type alias gives a new name to a type you already have (00 §8.4). The name is transparent: everywhere the old type is accepted, so is the new one.

### Command 1

```lagom
a type called score is a number
make final score equal to 97 of type score
say "{final score}"
```

Terminal transcript:

```sh
$ lagom run alias1.lagom
97
```

### Command 2

```lagom
a type called score is a number
a type called scores is a list of score
make xs equal to a list of 4, 9 of type scores
say "{first of xs}"
```

Terminal transcript:

```sh
$ lagom run alias2.lagom
4
```

### Command 3

```lagom
a type called score is a number
a type called points is a score
make p equal to 8 of type points
say "{p}"
```

Terminal transcript:

```sh
$ lagom run alias3.lagom
8
```

### Command 4

```lagom
make s equal to 5 of type score
a type called score is a number
say "{s}"
```

Terminal transcript:

```sh
$ lagom run alias4.lagom
5
```

### Command 5

```lagom
a type called score is a number
make a equal to 3 of type score
make b equal to 4 of type score
say "{a plus b}"
```

Terminal transcript:

```sh
$ lagom run alias5.lagom
7
```

---

## Quick reference: the five most important first-day forms

A first program combines five forms from this catalog: ask for input, make a value, an `if` branch, a `repeat` loop, and `say` output.
Terminal transcript:

```sh
$ lagom new hello
$ cd hello
$ lagom run main.lagom
What is your name?: Ada
Hello, Ada!
```

---

## Notes

- The transcripts above show the **printed output** a `.lagom` program emits when run with `lagom run`. They are examples, not a single program.
- Some sections are labeled **M1+** or **Expert**: those forms are documented syntax planned for later milestones or for advanced systems programming.
- When a command can print nothing, the transcript shows no printed lines after the `lagom run` command. When an optional value is absent, the printed word is `nothing`.
