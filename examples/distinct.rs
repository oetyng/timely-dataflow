extern crate timely;
extern crate columnar;

use std::fmt::Debug;
use std::hash::Hash;

use timely::communication::{Data, Communicator, ThreadCommunicator};
use timely::progress::timestamp::RootTimestamp;
use timely::progress::nested::Summary::Local;
use timely::example::*;
use timely::example::distinct::DistinctExtensionTrait;
use timely::example::builder::{Graph, Root, SubgraphBuilder};

use columnar::Columnar;

fn main() {
    _distinct(ThreadCommunicator);
}

fn _distinct<C: Communicator>(communicator: C) {

    let mut root = Root::new(communicator);

    let (mut input1, mut input2) = {
        let borrow = root.builder();
        let mut graph = SubgraphBuilder::new(&borrow);
        let (input1, input2) = {
            let builder = graph.builder();

            // try building some input scopes
            let (input1, mut stream1) = builder.new_input::<u64>();
            let (input2, mut stream2) = builder.new_input::<u64>();

            // prepare some feedback edges
            let (mut feedback1, mut feedback1_output) = builder.feedback(RootTimestamp::new(1000000), Local(1));
            let (mut feedback2, mut feedback2_output) = builder.feedback(RootTimestamp::new(1000000), Local(1));

            // build up a subgraph using the concatenated inputs/feedbacks
            let (mut egress1, mut egress2) = _create_subgraph(&mut stream1.concat(&mut feedback1_output),
                                                              &mut stream2.concat(&mut feedback2_output));

            // connect feedback sources. notice that we have swapped indices ...
            feedback1.connect_input(&mut egress2);
            feedback2.connect_input(&mut egress1);

            (input1, input2)
        };
        graph.seal();

        (input1, input2)
    };

    root.step();

    // move some data into the dataflow graph.
    input1.send_messages(&RootTimestamp::new(0), vec![1u64]);
    input2.send_messages(&RootTimestamp::new(0), vec![2u64]);

    // see what everyone thinks about that ...
    root.step();

    input1.advance(&RootTimestamp::new(0), &RootTimestamp::new(1000000));
    input2.advance(&RootTimestamp::new(0), &RootTimestamp::new(1000000));
    input1.close_at(&RootTimestamp::new(1000000));
    input2.close_at(&RootTimestamp::new(1000000));

    // spin
    while root.step() { }
}

fn _create_subgraph<'a, G, D>(source1: &mut Stream<'a, G, D>, source2: &mut Stream<'a, G, D>) ->
    (Stream<'a, G, D>, Stream<'a, G, D>)
where G: Graph+'a, D: Data+Hash+Eq+Debug+Columnar,
      G::Timestamp: Hash {

    let mut subgraph = SubgraphBuilder::<_, u64>::new(source1.graph);
    let result = {
        let subgraph_builder = subgraph.builder();

        (
            source1.enter(&subgraph_builder).distinct().leave(),
            source2.enter(&subgraph_builder).leave()
        )
    };
    subgraph.seal();

    result
}
