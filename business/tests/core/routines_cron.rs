//! Cron expressions of routines (see "Cron" in `docs/routines-format.md`): the grammar, names and
//! Sunday as 0 or 7, the Vixie OR rule for the two day fields, and the calendar rule that refuses
//! an expression that never fires.

use harness_business::routines::cron::CronExpr;

/// Weekday numbers, Sunday 0.
const SUN: i8 = 0;
const MON: i8 = 1;
const WED: i8 = 3;
const FRI: i8 = 5;
const SAT: i8 = 6;

#[test]
fn the_grammar_accepts_the_five_field_forms() {
    for expression in [
        "* * * * *",
        "*/15 9-17 * * mon-fri",
        "0,30 8 1,15 * *",
        "5 4 * jan-jun,sep *",
        "0 9 * * fri-sun,mon",
        "0 9 * * 5-0",
        "0 3 * * 7",
        "00 09 * * *",
        "0-59/20 * * * *",
        "0 0 29 2 *",
        "0 12 * DEC Sat",
        "0 0 * * */2",
    ] {
        assert!(CronExpr::parse(expression).is_ok(), "{expression} was refused");
    }
}

#[test]
fn the_grammar_refuses_everything_else() {
    for expression in [
        "",
        "* * * *",
        "* * * * * *",
        " * * * * *",
        "* * * * * ",
        "*  * * * *",
        "*\t* * * *",
        "@daily",
        "60 * * * *",
        "* 24 * * *",
        "* * 0 * *",
        "* * 32 * *",
        "* * * 13 *",
        "* * * * 8",
        "5/10 * * * *",
        "*/0 * * * *",
        "*/60 * * * *",
        "*/100 * * * *",
        "30-10 * * * *",
        "* * * * fri-mon",
        "* * * * mon-",
        "1,,2 * * * *",
        "? * * * *",
        "* * L * *",
        "* * 15W * *",
        "* * * * 5#2",
        "jan * * * *",
        "* * * * monday",
        "* * * * -1",
        "007 * * * *",
        "* * * * +1",
    ] {
        assert!(CronExpr::parse(expression).is_err(), "{expression:?} was accepted");
    }
}

#[test]
fn names_are_case_insensitive_and_sunday_is_zero_or_seven() {
    let names = CronExpr::parse("0 9 * Jan-MAR sun").unwrap();
    let numbers = CronExpr::parse("0 9 * 1-3 7").unwrap();

    for (month, weekday) in [(1, SUN), (3, SUN), (4, SUN), (2, MON)] {
        assert_eq!(
            names.matches_date(month, 10, weekday),
            numbers.matches_date(month, 10, weekday),
            "month {month}, weekday {weekday}"
        );
    }

    assert!(names.matches_date(2, 10, SUN));
    assert!(!names.matches_date(4, 10, SUN));

    let weekend = CronExpr::parse("0 9 * * fri-sun").unwrap();
    let matching: Vec<i8> = (0..7).filter(|day| weekend.matches_date(6, 10, *day)).collect();
    assert_eq!(matching, [SUN, FRI, SAT]);

    // Sunday ends a range as 7 only when the range starts after Sunday.
    for sunday_only in ["0 9 * * sun-sun", "0 9 * * 0-0"] {
        let cron = CronExpr::parse(sunday_only).unwrap();
        let matching: Vec<i8> = (0..7).filter(|day| cron.matches_date(6, 10, *day)).collect();
        assert_eq!(matching, [SUN], "{sunday_only}");
    }
    let numbers = CronExpr::parse("0 9 * * 5-0").unwrap();
    let matching: Vec<i8> = (0..7).filter(|day| numbers.matches_date(6, 10, *day)).collect();
    assert_eq!(matching, [SUN, FRI, SAT]);
}

#[test]
fn both_restricted_day_fields_match_either_one() {
    let either = CronExpr::parse("30 6 1,15 * mon").unwrap();

    assert!(either.matches_date(5, 1, WED), "the 1st on a Wednesday");
    assert!(either.matches_date(5, 20, MON), "a Monday that is not the 1st or 15th");
    assert!(!either.matches_date(5, 20, WED));
}

#[test]
fn a_day_field_starting_with_a_star_combines_with_and() {
    let both = CronExpr::parse("0 0 1-7 * */1").unwrap();
    assert!(both.matches_date(5, 3, WED), "*/1 is every weekday");
    assert!(!both.matches_date(5, 9, WED));

    let even = CronExpr::parse("0 0 1-7 * */2").unwrap();
    assert!(!even.matches_date(5, 3, WED), "*/2 still begins with a star: AND");
    assert!(even.matches_date(5, 3, SUN));

    let only_days = CronExpr::parse("0 0 13 * *").unwrap();
    assert!(only_days.matches_date(8, 13, FRI) && !only_days.matches_date(8, 14, SAT));
}

#[test]
fn an_expression_that_never_fires_is_refused_by_the_calendar() {
    for expression in ["0 0 31 2 *", "0 0 30 2 *", "0 0 31 4,6,9,11 *", "0 0 31 apr *"] {
        assert!(CronExpr::parse(expression).is_err(), "{expression} was accepted");
    }

    for expression in ["0 0 29 2 *", "0 0 31 2,3 *", "0 0 31 2 mon", "0 0 30-31 1 *"] {
        assert!(CronExpr::parse(expression).is_ok(), "{expression} was refused");
    }
}

#[test]
fn times_come_in_order() {
    let expr = CronExpr::parse("45,5 23,1 * * *").unwrap();

    assert_eq!(expr.times().collect::<Vec<_>>(), [(1, 5), (1, 45), (23, 5), (23, 45)]);
}
