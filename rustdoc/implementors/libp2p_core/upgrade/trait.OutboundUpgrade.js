(function() {var implementors = {
"libp2p":[],
"libp2p_core":[],
"libp2p_noise":[["impl&lt;T&gt; <a class=\"trait\" href=\"libp2p_core/upgrade/trait.OutboundUpgrade.html\" title=\"trait libp2p_core::upgrade::OutboundUpgrade\">OutboundUpgrade</a>&lt;T&gt; for <a class=\"struct\" href=\"libp2p_noise/struct.Config.html\" title=\"struct libp2p_noise::Config\">Config</a><span class=\"where fmt-newline\">where\n    T: <a class=\"trait\" href=\"futures_io/if_std/trait.AsyncRead.html\" title=\"trait futures_io::if_std::AsyncRead\">AsyncRead</a> + <a class=\"trait\" href=\"futures_io/if_std/trait.AsyncWrite.html\" title=\"trait futures_io::if_std::AsyncWrite\">AsyncWrite</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> + 'static,</span>"]],
"libp2p_swarm":[["impl&lt;T: <a class=\"trait\" href=\"libp2p_swarm/handler/trait.OutboundUpgradeSend.html\" title=\"trait libp2p_swarm::handler::OutboundUpgradeSend\">OutboundUpgradeSend</a>&gt; <a class=\"trait\" href=\"libp2p_core/upgrade/trait.OutboundUpgrade.html\" title=\"trait libp2p_core::upgrade::OutboundUpgrade\">OutboundUpgrade</a>&lt;<a class=\"struct\" href=\"libp2p_swarm/struct.Stream.html\" title=\"struct libp2p_swarm::Stream\">Stream</a>&gt; for <a class=\"struct\" href=\"libp2p_swarm/handler/struct.SendWrapper.html\" title=\"struct libp2p_swarm::handler::SendWrapper\">SendWrapper</a>&lt;T&gt;"]],
"libp2p_yamux":[["impl&lt;C&gt; <a class=\"trait\" href=\"libp2p_core/upgrade/trait.OutboundUpgrade.html\" title=\"trait libp2p_core::upgrade::OutboundUpgrade\">OutboundUpgrade</a>&lt;C&gt; for <a class=\"struct\" href=\"libp2p_yamux/struct.Config.html\" title=\"struct libp2p_yamux::Config\">Config</a><span class=\"where fmt-newline\">where\n    C: <a class=\"trait\" href=\"futures_io/if_std/trait.AsyncRead.html\" title=\"trait futures_io::if_std::AsyncRead\">AsyncRead</a> + <a class=\"trait\" href=\"futures_io/if_std/trait.AsyncWrite.html\" title=\"trait futures_io::if_std::AsyncWrite\">AsyncWrite</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> + 'static,</span>"]]
};if (window.register_implementors) {window.register_implementors(implementors);} else {window.pending_implementors = implementors;}})()