use nom::{
    error::ParseError, IResult, Parser,
};

pub fn consume_count<I, O, E, F, G>(mut f: F, mut g : G, count: usize) -> impl FnMut(I) -> IResult<I, (), E>
where
    I: Clone + PartialEq,
    F: Parser<I, O, E>,
    G: FnMut(usize, O) -> (),
    E: ParseError<I>,
{
    move |i: I| {
        let mut input = i.clone();

        for idx in 0..count {
            let input_ = input.clone();
            match f.parse(input_) {
                Ok((i, o)) => {
                    g(idx, o);
                    input = i;
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }

        Ok((input, ()))
    }
}
