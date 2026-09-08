# Lagom Syntax Demo Catalog

This is a reference catalog of Lagom's documented surface syntax. **Every section has five independent examples.** The snippets are examples, not one program: copy the examples you want into a `.lagom` file rather than compiling this entire Markdown document as one program. Some forms are planned for later milestones and are labeled accordingly.

## Status legend

- **M0** — current student-layer syntax represented by the compiler and validation programs.
- **M1+** — documented syntax planned for a later milestone.
- **Expert** — documented low-level syntax for advanced systems programming.
- **Lexical** — source-writing syntax such as comments, names, and indentation.

---

## 1. `#` comments — Lexical

A `#` comment runs to the end of the line and is ignored by the compiler.

```lagom
# This is a complete-line comment.

say "hello" # This explains the output.

make age equal to 12 # The comment does not change the value.

# Comments can document a small decision.
make changing tries equal to 3 # Three attempts are allowed.

if score is greater than 90 # Choose the winning branch.
    say "excellent"
```

## 2. `##` documentation comments — Lexical / M1+

A `##` comment attaches documentation to the following declaration.

```lagom
## Greets one person.
function greet
    takes text called name
    say "Hello, {name}!"

## Adds two whole numbers.
function add
    takes number called left
    takes number called right
    give back left plus right

## A question used by the quiz.
structure question
    has prompt of type text
    has answer of type text

## Runs the empty-list lesson.
test "first item"
    check that first of a list of 7 is equal to 7

## A reusable score value.
make top score equal to 100
```

## 3. Indentation blocks — Lexical / M0

Lagom uses indentation instead of braces or `end` keywords. Four spaces are canonical.

```lagom
if ready is equal to true
    say "start"

repeat while count is less than 3
    increase count by 1

function welcome
    takes text called name
    say "Welcome, {name}!"

structure book
    has title of type text
    has pages of type number

if score is greater than 50
    if score is at most 100
        say "valid score"
```

## 4. Continuation lines — Lexical / M0

An incomplete expression can continue on a deeper-indented line.

```lagom
make greeting equal to "Hello, " plus
    name

make total equal to first plus second plus
    third

make message equal to "The answer is " plus
    text from answer

make average equal to total divided by
    count

make sentence equal to "A long sentence " plus
    "can be split across lines."
```

## 5. Multi-word names — Lexical / M0

Names may contain multiple lowercase words when the parse remains deterministic.

```lagom
make first name equal to "Ada"

make high score equal to 100

make changing lamp oil equal to 10

say first name

set lamp oil to lamp oil minus 1
```

## 6. `say` — Output / M0

```lagom
say "Hello, world!"

say 42

say true

say "The score is {score}."

say first of names
```

## 7. `ask` — Input / M0

`ask` reads text and optionally displays a prompt.

```lagom
make name equal to ask "What is your name?"

make answer equal to ask "Answer: "

make command equal to ask "go north, or look? "

make color equal to ask "Favorite color: "

say "You chose {ask \"Choose a door: \"}."
```

## 8. `make` — Immutable declarations / M0

```lagom
make age equal to 15

make greeting equal to "Hello"

make total equal to left plus right

make names equal to a list of "Ana", "Bo", "Cy"

make answer equal to ask "What is 2 plus 2? "
```

## 9. `make changing` — Mutable declarations / M0

```lagom
make changing score equal to 0

make changing attempts equal to 3

make changing found equal to false

make changing current room equal to "entrance"

make changing total equal to 0 of type number
```

## 10. `equal to` — Initialization and equality / M0

`equal to` initializes a name in `make`; with `is`, it compares two values.

```lagom
make message equal to "Hello from Lagom"

make answer equal to 6 times 7

make changing lives equal to 3

if answer is equal to 42
    say "correct"

if name is not equal to ""
    say "a name was entered"
```

## 11. `set ... to` — Assignment / M0

`set` changes an existing mutable binding or a mutable target.

```lagom
make changing score equal to 0
set score to 10

make changing name equal to "old"
set name to "new"

make changing found equal to false
set found to true

make changing things equal to a list of 1, 2, 3
set things at 1 to 99

set count of player to count of player plus 1
```

## 12. `increase ... by` — Compound addition / M0

```lagom
make changing score equal to 0
increase score by 10

make changing level equal to 1
increase level by 1

make changing total equal to 100
increase total by bonus

make changing lamp oil equal to 10
increase lamp oil by refill amount

increase score of player by 5
```

## 13. `decrease ... by` — Compound subtraction / M0

```lagom
make changing lives equal to 3
decrease lives by 1

make changing fuel equal to 20
decrease fuel by 5

make changing score equal to 100
decrease score by penalty

make changing lamp oil equal to 10
decrease lamp oil by 1

decrease health of player by damage
```

## 14. Symbolic arithmetic — M0

The four school-math symbols are supported.

```lagom
make sum equal to 7 + 5

make difference equal to 20 - 8

make product equal to 6 * 9

make quotient equal to 20 / 4

make total equal to base + bonus * multiplier
```

## 15. Word arithmetic — M0

Word aliases make arithmetic read like a sentence.

```lagom
make sum equal to 7 plus 5

make difference equal to 20 minus 8

make product equal to 6 times 9

make quotient equal to 20 divided by 4

make total equal to base plus bonus times multiplier
```

## 16. Division and remainder forms — M0

```lagom
make exact equal to 20 divided by 4

make fraction equal to 5 divided by 2

make whole equal to 5 divided evenly by 2

make leftover equal to remainder of 17 and 5

make same leftover equal to 17 modulo 5
```

## 17. Comparisons — M0

```lagom
if age is greater than 13
    say "teenager or older"

if score is less than 50
    say "keep practicing"

if score is at least 80
    say "passing"

if temperature is at most 0
    say "freezing"

if name is not equal to ""
    say "name entered"
```

## 18. Boolean logic — M0

```lagom
if age is greater than 13 and has permission
    say "allowed"

if is weekend or is holiday
    say "no school"

if not finished
    say "keep going"

if score is at least 80 and score is at most 100
    say "valid passing score"

if not banned and (is teacher or is administrator)
    say "staff access"
```

## 19. Literals — M0

```lagom
make whole equal to 42

make precise equal to 3.5

make greeting equal to "hello"

make enabled equal to true

make missing equal to nothing
```

## 20. Parenthesized expressions — M0

Parentheses make grouping explicit.

```lagom
make total equal to (base plus bonus) times multiplier

make average equal to total divided by (count plus 1)

if (age is greater than 13 and has permission) or is administrator
    say "allowed"

make answer equal to bigger of (left plus 1) and (right plus 1)

make safe equal to not (is blocked or is banned)
```

## 21. Text and interpolation — M0

```lagom
make greeting equal to "Hello, {name}!"

make report equal to "Score: {score} out of {total}."

make location equal to "{row}, {column}"

make message equal to "{name} has {lives} lives left."

say "The result of {left} plus {right} is {left plus right}."
```

## 22. Text conversion calls — M0

These are ordinary readable calls; parsing from text can fail.

```lagom
attempt number from answer if it fails then
    say "Please enter a whole number."

attempt decimal from measurement if it fails then
    say "Please enter a decimal number."

make count equal to attempt number from text count

make formatted equal to text from score

say "You entered {text from number of items} items."
```

## 23. `if` — Conditional branches / M0

```lagom
if raining
    say "take an umbrella"

if score is greater than 90
    say "excellent"

if answer is equal to secret
    say "correct"

if lamp oil is at most 0
    say "the lamp is out"

if name is equal to "Ada"
    say "welcome, Ada"
```

## 24. `otherwise if` — Multiple conditions / M0

```lagom
if score is at least 90
    say "A"
otherwise if score is at least 80
    say "B"

if temperature is greater than 30
    say "hot"
otherwise if temperature is greater than 15
    say "mild"

if move is equal to "north"
    say "you go north"
otherwise if move is equal to "look"
    say "you look around"

if age is less than 5
    say "toddler"
otherwise if age is less than 13
    say "child"

if status is equal to "new"
    say "created"
otherwise if status is equal to "open"
    say "already open"
```

## 25. `otherwise` — Fallback branches / M0

```lagom
if found
    say "found"
otherwise
    say "not found"

if score is at least 50
    say "pass"
otherwise
    say "try again"

if command is equal to "quit"
    stop
otherwise
    say "unknown command"

if light is equal to true
    say "bright"
otherwise
    say "dark"

if answer is equal to expected
    say "correct"
otherwise if answer is equal to close answer
    say "nearly"
otherwise
    say "not quite"
```

## 26. `repeat ... times using` — Counting loops / M0

```lagom
repeat 5 times using i
    say i

repeat 10 times using turn
    say "turn {turn}"

repeat 3 times using attempt
    say "attempt {attempt}"

repeat 4 times using row
    say "row {row}"

repeat 2 times using copy
    say "copy {copy}"
```

## 27. `repeat while` — Condition loops / M0

```lagom
repeat while score is less than 100
    increase score by 10

repeat while found is equal to false
    make changing guess equal to ask "guess: "

repeat while fuel is greater than 0
    decrease fuel by 1

repeat while command is not equal to "quit"
    set command to ask "command: "

repeat while size of queue is greater than 0
    say first of queue
```

## 28. `repeat for each ... in` — Collection loops / M0

```lagom
repeat for each name in names
    say "Hello, {name}!"

repeat for each score in scores
    increase total by score

repeat for each room in rooms
    say name of room

repeat for each word in words
    say word

repeat for each card in cards
    say prompt of card
```

## 29. `stop` — Leave a loop / M0

```lagom
repeat while true
    if answer is equal to secret
        stop

repeat for each item in things
    if item is equal to target
        stop

repeat 100 times using turn
    if health is at most 0
        stop

repeat while fuel is greater than 0
    decrease fuel by 1
    if engine started
        stop

repeat while command is not equal to "quit"
    set command to ask "command: "
    if command is equal to "quit"
        stop
```

## 30. `next` — Skip to the next iteration / M0

```lagom
repeat for each number in numbers
    if number is less than 0
        next
    say number

repeat 10 times using i
    if i is equal to 5
        next
    say i

repeat for each name in names
    if name is equal to ""
        next
    say name

repeat while size of queue is greater than 0
    make item equal to first of queue
    if item is nothing
        next
    say item

repeat for each score in scores
    if score is at most 0
        next
    increase total by score
```

## 31. Lists — M0

```lagom
make fruits equal to a list of "apple", "pear", "plum"

make numbers equal to a list of 1, 2, 3, 4

make empty list equal to a list of

make players equal to a list of "Ana", "Bo", "Cy"

make mixed values equal to a list of 7, 3.5, "seven"
```

## 32. Maps — M0

```lagom
make ages equal to a map from "Ana" to 11, "Bo" to 12

make rooms equal to a map from "entrance" to "library", "library" to "vault"

make prices equal to a map from "apple" to 2, "pear" to 3

make scores equal to a map from "red" to 10, "blue" to 20

make directions equal to a map from "north" to "up", "south" to "down"
```

## 33. Pairs — M0

```lagom
make point equal to a pair of 3 and 4

make name and score equal to a pair of "Ada" and 100

make bounds equal to a pair of 0 and 10

make answer and correct equal to a pair of "yes" and true

make origin equal to a pair of 0 and 0
```

## 34. `first of` and `size of` — M0

```lagom
say first of names

make first name equal to first of names

say size of names

if size of queue is greater than 0
    say first of queue

make remaining equal to size of cards minus 1
```

## 35. `at` indexing and lookup — M0

```lagom
say things at 0

make third equal to numbers at 2

say rooms at "entrance"

make letter equal to greeting at 1

make current equal to halls at where
```

## 36. `set ... at ... to` — Indexed assignment / M0

```lagom
make changing numbers equal to a list of 1, 2, 3
set numbers at 0 to 10

make changing names equal to a list of "old", "new"
set names at 1 to "updated"

make changing ages equal to a map from "Ana" to 11
set ages at "Ana" to 12

make changing letters equal to a list of "a", "b", "c"
set letters at 2 to "z"

set inventory at "keys" to 4
```

## 37. `function` declarations — M0

```lagom
function greet
    takes text called name
    say "Hello, {name}!"

function add
    takes number called left
    takes number called right
    give back left plus right

function show score
    takes number called score
    say "Score: {score}"

function is adult
    takes number called age
    give back age is at least 18

function repeat greeting
    takes text called name
    say "Welcome, {name}!"
```

## 38. `takes` parameters — M0

```lagom
function greet
    takes text called name
    say "Hello, {name}!"

function add
    takes number called left
    takes number called right
    give back left plus right

function calculate score
    takes number of correct answers
    takes number of total questions
    give back correct answers divided evenly by total questions

function print names
    takes a list of text called names
    repeat for each name in names
        say name

function describe point
    takes a pair of number and number called point
    say point
```

## 39. `returns` clauses — M0

```lagom
function add
    takes number called left
    takes number called right
    returns a number
    give back left plus right

function greeting
    takes text called name
    returns text
    give back "Hello, {name}!"

function average
    takes a list of numbers called scores
    returns a number
    give back first of scores

function is valid
    takes number called score
    returns a boolean
    give back score is at least 0

function read name
    returns text
    give back ask "Name: "
```

## 40. `give back` — Return values / M0

```lagom
function double
    takes number called value
    give back value times 2

function absolute
    takes number called value
    if value is less than 0
        give back -value
    give back value

function choose name
    takes boolean called formal
    if formal
        give back "Doctor"
    give back "Friend"

function first score
    takes a list of numbers called scores
    give back first of scores

function make greeting
    takes text called name
    give back "Hello, {name}!"
```

## 41. `can fail` — Fallible functions / M0

```lagom
function divide
    takes number called top
    takes number called bottom
    returns a decimal
    can fail
    if bottom is equal to 0
        fail with "cannot divide by zero"
    give back top divided by bottom

function parse score
    takes text called answer
    returns a number
    can fail
    give back number from answer

function read config
    returns text
    can fail
    give back read file at "config.txt"

function open door
    takes text called key
    can fail
    if key is not equal to "gold"
        fail with "wrong key"

function find player
    takes text called name
    returns a player
    can fail
    give back players at name
```

## 42. `fail with` — Failure values / M0

```lagom
function divide
    takes number called bottom
    can fail
    if bottom is equal to 0
        fail with "cannot divide by zero"

function parse age
    takes text called answer
    returns a number
    can fail
    attempt number from answer and pass the problem on

function open gate
    takes boolean called unlocked
    can fail
    if not unlocked
        fail with "the gate is locked"

function find item
    takes text called name
    can fail
    if name is equal to ""
        fail with "an item needs a name"

function connect
    takes text called address
    can fail
    if address is equal to ""
        fail with "address is empty"
```

## 43. `attempt ... if it fails then ... otherwise` — Error handling / M0

```lagom
attempt number from answer if it fails then
    say "That was not a number."
otherwise
    say "You entered {result}."

attempt divide 10 and 0 if it fails then
    say problem
otherwise
    say result

attempt read file at "notes.txt" if it fails then
    say "Could not read the notes."
otherwise
    say result

attempt parse score text score if it fails then
    set score to 0
otherwise
    set score to result

attempt find player "Ada" if it fails then
    say "Player not found."
otherwise
    say name of result
```

## 44. `attempt ... as` — Named failures / M1+

```lagom
attempt open file at path as problem
    say "File error: {problem}"
otherwise
    say result

attempt parse configuration as parse problem
    say "Configuration error: {parse problem}"
otherwise
    say result

attempt connect address as network problem
    say "Network error: {network problem}"
otherwise
    say "Connected."

attempt divide top and bottom as math problem
    say math problem
otherwise
    say result

attempt load profile as load problem
    say "Profile error: {load problem}"
otherwise
    say result
```

## 45. `and pass the problem on` — Error propagation / M1+

```lagom
function load score
    takes text called path
    returns a number
    can fail
    attempt read file at path and pass the problem on
    attempt number from result and pass the problem on
    give back result

function load name
    returns text
    can fail
    attempt read file at "name.txt" and pass the problem on
    give back result

function parse answer
    takes text called answer
    returns a number
    can fail
    attempt number from answer and pass the problem on
    give back result

function read settings
    returns text
    can fail
    attempt read file at "settings.txt" and pass the problem on
    give back result

function load level
    takes text called path
    returns a number
    can fail
    attempt load score path and pass the problem on
    give back result
```

## 46. `test` blocks — M0

```lagom
test "addition works"
    check that 2 plus 2 is equal to 4

test "a score passes"
    check that 90 is at least 50

test "the first list item exists"
    check that first of a list of 7 is equal to 7

test "the name is preserved"
    make name equal to "Ada"
    check that name is equal to "Ada"

test "the loop reaches five"
    make changing count equal to 0
    repeat 5 times using i
        increase count by 1
    check that count is equal to 5
```

## 47. `check that` — Test assertions / M0

```lagom
check that 1 plus 1 is equal to 2

check that score is at least 0

check that name is not equal to ""

check that size of names is greater than 0

check that found is equal to true
```

## 48. `use` modules — M0 / M1+

```lagom
use math for square root

use math for floor, square root

use drawing

use cards from "card-game/deck"

use text for split, join, trim
```

## 49. `of type` annotations — M1+

```lagom
make changing attempts equal to 0 of type number

make names equal to a list of "Ana" of type a list of text

make ratio equal to 0.5 of type decimal

make enabled equal to true of type boolean

make title equal to "Lagom" of type text
```

## 50. `structure` declarations — M0

```lagom
structure player
    has name of type text
    has score of type number

structure point
    has x of type number
    has y of type number

structure book
    has title of type text
    has pages of type number

structure room
    has name of type text
    has description of type text
    has north of type text

structure address
    has street of type text
    has city of type text
```

## 51. `has ... of type` fields — M0

```lagom
structure person
    has name of type text

structure counter
    has count of type number

structure switch
    has enabled of type boolean

structure scores
    has values of type a list of number

structure directory
    has entries of type a map from text to number
```

## 52. `with` construction — M0

```lagom
make player equal to a player with name "Ada" and score 10

make point equal to a point with x 3 and y 4

make room equal to a room with name "entrance" and description "A hall" and north "library"

make book equal to a book with title "Lagom" and pages 200

make address equal to an address with street "Main Street" and city "Uppsala"
```

## 53. Field reads — M0

```lagom
say name of player

say score of player

make title equal to title of book

say description of room

if enabled of switch is equal to true
    say "on"
```

## 54. Flowing calls — M0

Flowing calls are ordinary calls without parentheses.

```lagom
say first of things

make larger equal to bigger of left and right

say square root of 16

make item equal to things at 0

send "hello" to messages
```

## 55. Positional calls — M0

```lagom
greet "Ada"

add 2 and 3

divide 10 and 2

make answer equal to random from 1 to 100

print name and score
```

## 56. Labeled `with` arguments — M1+

```lagom
greet with name "Ada"

make point equal to a point with x 3 and y 4

open file with path "notes.txt" and mode "read"

make window equal to a new window with width 800 and height 600

format date with year 2026 and month 9 and day 7
```

## 57. `using` function arguments — M1+

```lagom
make doubled equal to map scores using double

make raised equal to map scores using it plus 5

make names equal to map people using name of it

make positive equal to keep numbers using it is greater than 0

make total equal to combine scores with start 0 using start plus it
```

## 58. `where` filters — M1+

```lagom
make passing equal to keep scores where it is at least 80

make adults equal to keep people where age of it is at least 18

make short names equal to keep names where size of it is less than 8

make available equal to keep rooms where open of it is equal to true

make positive equal to keep numbers where it is greater than 0
```

## 59. Function-valued lambdas — M1+

```lagom
make double equal to a function taking n
    give back n times 2

make greet equal to a function taking name
    give back "Hello, {name}!"

make square equal to a function taking n
    give back n times n

make is adult equal to a function taking age
    give back age is at least 18

make add one equal to a function taking value
    give back value plus 1
```

## 60. `taking ... giving back` inline lambdas — M1+

```lagom
make doubled equal to map numbers using taking value giving back value times 2

make names equal to map people using taking person giving back name of person

make squares equal to map numbers using taking n giving back n times n

make adults equal to keep ages using taking age giving back age is at least 18

make total equal to combine numbers with start 0 using taking start and it giving back start plus it
```

## 61. Option values — M0 / M1+

```lagom
make maybe name equal to first of names

if maybe name is nothing
    say "The list is empty."

make maybe score equal to first of scores

if maybe score is something
    say "A score exists."

make title equal to first of titles of type text?
```

## 62. Option type spellings — M1+

```lagom
make name equal to nothing of type text?

make score equal to nothing of type a number or nothing

make result equal to first of scores

make child equal to nothing of type a person or nothing

make item equal to nothing of type a list of text?
```

## 63. `kind` declarations — M1+

```lagom
kind light
    is a red
    is a yellow
    is a green

kind result
    is a success with value of type text
    is a failure with message of type text

kind shape
    is a circle with radius of type number
    is a rectangle with width of type number and height of type number
    is a blank

kind command
    is a move with direction of type text
    is a look
    is a quit

kind answer
    is a yes
    is a no
```

## 64. `match` and `when` — M1+

```lagom
match light
    when red
        say "stop"
    when yellow
        say "wait"
    when green
        say "go"

match shape
    when a circle with radius r
        say "circle of radius {r}"
    when blank
        say "empty"

match answer
    when yes
        say "accepted"
    when no
        say "rejected"

match maybe name
    when nothing
        say "missing"
    when something with value name
        say name

match result
    when a success with value value
        say value
    when a failure with message message
        say message
```

## 65. `class` declarations — M1+

```lagom
class counter
    has count of type number

class document
    has title of type text

class player
    has name of type text
    has score of type number

class connection
    has address of type text

class animal
    has name of type text
```

## 66. `can` methods and `myself` — M1+

```lagom
class counter
    has count of type number
    can reset
        set count of myself to 0

class counter
    has count of type number
    can bump
        increase count of myself by 1

class player
    has score of type number
    can add points
        increase score of myself by 10

class document
    has title of type text
    can rename
        set title of myself to "updated"

class lamp
    has on of type boolean
    can turn off
        set on of myself to false
```

## 67. `construction` and `a new` — M1+

```lagom
class counter
    has count of type number
    construction
        takes number called starting count
        set count of myself to starting count

make counter equal to a new counter with starting count 0

make window equal to a new window with width 800 and height 600

make user equal to a new user with name "Ada"

make point equal to a new point with x 3 and y 4
```

## 68. `before last reference disappears` — M1+

```lagom
class document
    before last reference disappears
        say "document released"

class file handle
    before last reference disappears
        close file of myself

class connection
    before last reference disappears
        disconnect myself

class temporary folder
    before last reference disappears
        remove folder of myself

class lock guard
    before last reference disappears
        release lock of myself
```

## 69. `extends` inheritance — M1+

```lagom
class dog extends animal
    can speak
        say "Woof"

class cat extends animal
    can speak
        say "Meow"

class electric car extends car
    can charge
        say "charging"

class square extends shape
    can area
        give back side times side

class administrator extends user
    can manage users
        say "managing users"
```

## 70. `interface` and `does` — M2+

```lagom
interface drawable
    can draw

class circle does drawable
    can draw
        say "drawing circle"

interface printable
    can print

class report does printable
    can print
        say "printing report"

class widget does drawable, printable
    can draw
        say "drawing widget"
    can print
        say "printing widget"
```

## 71. Generic `anything` — M1+

```lagom
function first item
    takes a list of anything called items
    returns anything
    give back items at 0

function identity
    takes anything called value
    returns anything
    give back value

function show item
    takes anything called value
    say value

function replace first
    takes a list of anything called items
    takes anything called value
    set items at 0 to value

function pair values
    takes anything called left
    takes anything called right
    give back a pair of left and right
```

## 72. Generic `some type` — M2+

```lagom
function first item
    takes a list of some type called items
    returns some type
    give back items at 0

function box value
    takes some type called value
    returns a box of some type
    give back a new box with value value

function copy list
    takes a list of some type called items
    returns a list of some type
    give back items

function choose
    takes some type called left
    takes some type called right
    returns some type
    give back left

function optional first
    takes a list of some type called items
    returns some type?
    give back first of items
```

## 73. Generic constraints — M2+

```lagom
function biggest item
    takes a list of some type that does comparable called items
    returns some type
    give back first of items

function sort values
    takes a list of some type that does comparable called values
    returns a list of some type
    give back values

function add values
    takes some type that does addable called left
    takes some type that does addable called right
    returns some type
    give back left plus right

function print value
    takes anything that does printable called value
    say value

function draw item
    takes anything that does drawable called item
    draw item
```

## 74. Function types — M1+

```lagom
make double equal to a function from number to number

make convert equal to a function from text to number that can fail

make combine equal to a function from number and number to number

make loader equal to a function from text to text that can fail

make fetcher equal to a function from text to text that can wait
```

## 75. `a box of` types — M1+

```lagom
make node equal to nothing of type a box of number

structure tree
    has value of type number
    has left of type a box of tree
    has right of type a box of tree

make child equal to a box of parent

function unwrap
    takes a box of some type called value
    returns some type
    give back value of value

make next node equal to a box of current node
```

## 76. `owned` and `borrowed` parameters — Expert / M3+

```lagom
function consume buffer
    takes an owned buffer called input
    returns an owned buffer
    give back input

function inspect buffer
    takes a borrowed buffer called input
    say size of input

function compare text
    takes a borrowed text called left
    takes a borrowed text called right
    give back left is equal to right

function move file
    takes an owned file called source
    returns an owned file
    give back source

function print view
    takes a borrowed text called value
    say value
```

## 77. `within owning` regions — Expert / M3+

```lagom
within owning
    make buffer equal to allocate buffer of size 1024

function process
    takes an owned buffer called input
    within owning
        say size of input

within owning
    make changing total equal to 0
    increase total by 1

function move values
    takes an owned list of number called values
    within owning
        give back values

within owning
    make result equal to build tree
    say result
```

## 78. `within arena` regions — Expert / M4+

```lagom
within arena frame allocator
    make points equal to a list of 1, 2, 3

within arena request arena
    make response equal to build response

function parse packet
    takes borrowed text called input
    within arena packet arena
        give back decode input

within arena scratch
    make changing buffer equal to allocate buffer of size 4096

within arena frame
    repeat 10 times using i
        make point equal to create point i
```

## 79. `using` resource scopes — M1+

```lagom
using open file at "notes.txt"
    say read file of myself

using open connection to address
    send request to connection

using acquire lock of resource
    update resource

using create temporary directory
    write files to directory

using open database at "data.db"
    run query on database
```

## 80. `start a task` — Structured concurrency / M2+

```lagom
start a task
    download "a" to "a.file"

start a task
    download "b" to "b.file"

start a task
    say "background calculation"

start a task
    send "hello" to messages

start a task keep going
    try optional cleanup
```

## 81. `wait for all tasks` — Structured concurrency / M2+

```lagom
start a task
    download "a" to "a.file"
wait for all tasks

start a task
    calculate first report
start a task
    calculate second report
wait for all tasks

start a task
    say "one"
start a task
    say "two"
wait for all tasks

start a task
    send first message to messages
start a task
    send second message to messages
wait for all tasks

start a task
    refresh cache
wait for all tasks
```

## 82. `in the background` — M2+

```lagom
start refresh cache in the background

start watch files in the background

start play music in the background

start send telemetry in the background

start update preview in the background
```

## 83. Channels, `send`, and `receive` — M2+

```lagom
make messages equal to a channel of text
send "hello" to messages
say receive from messages

make numbers equal to a channel of number
send 42 to numbers
say receive from numbers

make events equal to a channel of text
start a task
    send "ready" to events
say receive from events

make work equal to a channel of job
send job to work
make next job equal to receive from work

repeat for each message in messages
    say message
```

## 84. `can wait` — Async capability / M2+

```lagom
function download
    takes text called address
    returns text
    can wait
    give back fetch address

function fetch profile
    takes text called address
    returns text
    can fail
    can wait
    give back download address

function save response
    takes text called response
    can wait
    write response to file

function wait for message
    returns text
    can wait
    give back receive from messages

function refresh data
    can wait
    download "data" to "data.cache"
```

## 85. Shared fields and guards — M2+

```lagom
class tally
    has shared count of type number guarded by a lock
    can add one
        increase count of myself by 1

class queue
    has shared items of type a list of text guarded by a lock
    can add item
        add item to items of myself

class scoreboard
    has shared score of type number guarded by a lock
    can record point
        increase score of myself by 1

class cache
    has shared entries of type a map from text to text guarded by a lock
    can store value
        set entries of myself at key to value

class account
    has shared balance of type number guarded by a lock
    can deposit amount
        increase balance of myself by amount
```

## 86. Atomic operations — Expert / M4+

```lagom
unsafe because updating a lock-free counter
    make count equal to an atomic number

unsafe because reading a shared statistic
    make current equal to load count with relaxed ordering

unsafe because publishing a state transition
    compare and exchange state from waiting to running with acquire release ordering

unsafe because incrementing a reference counter
    increase atomic references by 1 with relaxed ordering

unsafe because coordinating worker shutdown
    store true at shutdown with release ordering
```

## 87. `at compile time` — Comptime / M3+

```lagom
at compile time
    make table equal to a list of 1, 4, 9, 16, 25

at compile time
    make version equal to "1.0.0"

at compile time
    function lookup
        takes number called index
        give back table at index

at compile time
    check that size of embedded data is greater than 0

at compile time
    make parsed pattern equal to parse regular expression "[a-z]+"
```

## 88. `from the C library` — FFI / M3+

```lagom
from the C library "libsqlite3"
    function sqlite3 libversion
        returns a C string

from the C library "libc"
    function strlen
        takes a C string called value
        returns a C number

from the C library "libm"
    function cos
        takes a C decimal called value
        returns a C decimal

from the C library "libc"
    function malloc
        takes a C number called size
        returns a C pointer

from the C library "libsystem"
    function system version
        returns a C string
```

## 89. `export` — C ABI exports / M3+

```lagom
export function lagom add
    takes a C number called left
    takes a C number called right
    returns a C number
    give back left plus right

export function lagom greet
    takes a C string called name
    returns a C string
    give back name

export function lagom version
    returns a C string
    give back "1.0"

export function lagom multiply
    takes a C decimal called left
    takes a C decimal called right
    returns a C decimal
    give back left times right

export structure packet
    has size of type a C number
```

## 90. `unsafe` regions — Expert / M3+

Every unsafe region should explain why it is necessary.

```lagom
unsafe because calling a C function that needs a raw buffer
    make p equal to a raw pointer to byte with size 64

unsafe because reading a device register
    say load byte from register

unsafe because using a manual allocator
    make memory equal to allocate 1024 bytes

unsafe because converting a C pointer to a Lagom pointer
    make value equal to reinterpret pointer as a raw pointer to number

unsafe because writing a memory-mapped value
    store 65 at address
```

## 91. Raw memory operations — Expert / M3+

```lagom
unsafe because allocating a byte buffer
    make buffer equal to allocate 128 bytes

unsafe because releasing a manually allocated buffer
    deallocate buffer

unsafe because storing a byte
    store 65 at pointer

unsafe because loading a byte
    make value equal to load byte from pointer

unsafe because freeing an arena
    free all at arena
```

## 92. C structures and fixed-size types — Expert / M4+

```lagom
C structure packet header
    has magic of type unsigned 16 bit number
    has length of type unsigned 16 bit number

C structure point
    has x of type signed 32 bit number
    has y of type signed 32 bit number

C structure color
    has red of type unsigned 8 bit number
    has green of type unsigned 8 bit number
    has blue of type unsigned 8 bit number

C structure measurement
    has value of type a 32 bit decimal
    is packed to 1 byte

C structure aligned record
    has value of type unsigned 64 bit number
    is aligned to 8 bytes
```

## 93. Packed and aligned layouts — Expert / M4+

```lagom
C structure header
    has kind of type unsigned 8 bit number
    has length of type unsigned 32 bit number
    is packed to 1 byte

C structure vector
    has x of type a 32 bit decimal
    has y of type a 32 bit decimal
    is aligned to 16 bytes

C structure disk record
    has id of type unsigned 64 bit number
    is packed to 1 byte

C structure cache line
    has value of type unsigned 64 bit number
    is aligned to 64 bytes

C structure network address
    has port of type unsigned 16 bit number
    is packed to 1 byte
```

## 94. SIMD vectors and intrinsics — Expert / M4+

```lagom
make values equal to 4 of a 32 bit decimal packed

make pixels equal to 4 of an unsigned 8 bit number packed

unsafe because using a target-specific CPU instruction
    cpu intrinsic "pause"

unsafe because using a vector instruction
    cpu intrinsic "add packed decimals"

make lanes equal to 4 of a 32 bit decimal packed
set lanes at 0 to 1.0
```

## 95. Assembly and linker controls — Expert / M5+

```lagom
unsafe because using a processor instruction
    run assembly on x86-64 "pause"

unsafe because reading a timestamp counter
    run assembly on x86-64 "rdtsc"

place startup function in section ".init"

place interrupt handler in section ".text.interrupts"

link with "custom_runtime"
```

## 96. Volatile and freestanding syntax — Expert / M6+

```lagom
make register equal to a volatile pointer to unsigned 32 bit number at address 1073741824

unsafe because reading a hardware register
    make status equal to load register

unsafe because writing a hardware register
    store value at register

build with no runtime

link with "board_support"
```

---

## Quick reference: the five most important first-day forms

```lagom
say "Hello, world!"

make name equal to ask "What is your name?"

make changing score equal to 0

if score is greater than 0
    say "You have points."

repeat 3 times using i
    say i
```

## Notes

- The snippets intentionally use the canonical word forms from the architecture document.
- Arithmetic symbols remain valid aliases, while comparison and logic words are the preferred beginner spelling.
- `same as`, `different from`, `either`, and dot syntax are not included because they were rejected from the frozen language design.
- The current M0 compiler supports the student-layer subset; later sections preserve the planned syntax so this file can serve as a long-term language demo catalog.