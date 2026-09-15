//! The reserved words (7.0.3: "user names may not collide with them") —
//! printed by `lagom words`. Kept in step with the lexer's keyword table
//! (docs/13's M0 grammar plus the frozen phrase-tokens of 7.15).

pub const RESERVED_LISTING: &str = "\
reserved words (user names may not use these):

declarations & statements
  make  changing  equal to  of type  set  to  increase  decrease  by
  if  otherwise if  otherwise  repeat  times  using  while  for each  in
  stop  next  function  takes  called  returns  give back  fail with
  can fail  attempt  and pass the problem on  if it fails  then  as
  test  check that  use  for  structure  has

expressions
  and  or  not  is
  is equal to  is not equal to  is greater than  is less than
  is at least  is at most
  + - * /  plus  minus  divided by  divided evenly by  remainder of  modulo

literals & collections
  true  false  nothing
  a list of  a map from  a pair of

types & conversions
  number  decimal  text  boolean  random

options
  is nothing  is something
";
