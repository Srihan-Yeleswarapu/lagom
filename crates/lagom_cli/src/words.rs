// Copyright (c) 2026 Srihan Yeleswarapu.
// Source available under the Lagom License (LICENSE.md).

//! The reserved words (7.0.3: "user names may not collide with them") —
//! printed by `lagom words`. Kept in step with the lexer's keyword table
//! (docs/13's M0 grammar plus the frozen phrase-tokens of 7.15).

use super::args::CliResult;

/// `lagom words` — the reserved words; `--types` prints the type surface
/// (the built-ins plus the 8.4 alias form) instead.
pub fn cmd_words(rest: &[String]) -> CliResult {
    if rest.iter().any(|a| a == "--types") {
        println!("{}", TYPE_ALIAS_LISTING);
    } else {
        println!("{}", RESERVED_LISTING);
    }
    Ok(())
}

pub const RESERVED_LISTING: &str = "\
reserved words (user names may not use these):

declarations & statements
  make  changing  equal to  of type  set  to  increase  decrease  by
  if  otherwise if  otherwise  repeat  times  using  while  for each  in
  stop  next  function  takes  called  returns  gives back  fail with
  can fail  attempt  and pass the problem on  if it fails  then  as
  test  check that  use  for  structure  has
  class  can  construction  a new  myself
  extends  interface  does
  before last reference disappears
  start a task  keep going  wait for all tasks

expressions
  and  or  not  is
  is equal to  is not equal to  is greater than  is less than
  is at least  is at most
  + - * /  plus  minus  divided by  divided evenly by  remainder of  modulo

literals & collections
  true  false  nothing
  a list of  a map from  a pair of  a channel of

types & conversions
  number  decimal  text  boolean  random

options
  is nothing  is something

tasks & channels
  send  receive
";

/// `lagom words --types`: the type surface — the built-ins every program
/// sees and the alias form that names new ones (8.4, listed because the
/// alias line is the one piece of type syntax a student writes by hand).
pub const TYPE_ALIAS_LISTING: &str = "\
types (use with `of type` and `takes … of type`):

built-in
  number  decimal  text  boolean
  a list of T  a map from K to V  a pair of A and B  a T or nothing
  a new C with … (10.2: makes an object of class C)

new names (8.4 type aliases)
  a type called <name> is a <type>
  — transparent: everywhere the old type is accepted, so is <name>.
  — order-free: an alias may be declared after the line that uses it.
";
