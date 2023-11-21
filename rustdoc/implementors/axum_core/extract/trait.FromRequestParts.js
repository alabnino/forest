(function() {var implementors = {
"axum":[["impl&lt;S&gt; <a class=\"trait\" href=\"axum/extract/trait.FromRequestParts.html\" title=\"trait axum::extract::FromRequestParts\">FromRequestParts</a>&lt;S&gt; for <a class=\"struct\" href=\"axum/extract/struct.OriginalUri.html\" title=\"struct axum::extract::OriginalUri\">OriginalUri</a><span class=\"where fmt-newline\">where\n    S: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Sync.html\" title=\"trait core::marker::Sync\">Sync</a>,</span>"],["impl&lt;S, T&gt; <a class=\"trait\" href=\"axum/extract/trait.FromRequestParts.html\" title=\"trait axum::extract::FromRequestParts\">FromRequestParts</a>&lt;S&gt; for <a class=\"struct\" href=\"axum/extract/struct.ConnectInfo.html\" title=\"struct axum::extract::ConnectInfo\">ConnectInfo</a>&lt;T&gt;<span class=\"where fmt-newline\">where\n    S: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Sync.html\" title=\"trait core::marker::Sync\">Sync</a>,\n    T: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/clone/trait.Clone.html\" title=\"trait core::clone::Clone\">Clone</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Sync.html\" title=\"trait core::marker::Sync\">Sync</a> + 'static,</span>"],["impl&lt;S&gt; <a class=\"trait\" href=\"axum/extract/trait.FromRequestParts.html\" title=\"trait axum::extract::FromRequestParts\">FromRequestParts</a>&lt;S&gt; for <a class=\"struct\" href=\"axum/extract/struct.WebSocketUpgrade.html\" title=\"struct axum::extract::WebSocketUpgrade\">WebSocketUpgrade</a>&lt;<a class=\"struct\" href=\"axum/extract/ws/struct.DefaultOnFailedUpdgrade.html\" title=\"struct axum::extract::ws::DefaultOnFailedUpdgrade\">DefaultOnFailedUpdgrade</a>&gt;<span class=\"where fmt-newline\">where\n    S: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Sync.html\" title=\"trait core::marker::Sync\">Sync</a>,</span>"],["impl&lt;OuterState, InnerState&gt; <a class=\"trait\" href=\"axum/extract/trait.FromRequestParts.html\" title=\"trait axum::extract::FromRequestParts\">FromRequestParts</a>&lt;OuterState&gt; for <a class=\"struct\" href=\"axum/extract/struct.State.html\" title=\"struct axum::extract::State\">State</a>&lt;InnerState&gt;<span class=\"where fmt-newline\">where\n    InnerState: <a class=\"trait\" href=\"axum/extract/trait.FromRef.html\" title=\"trait axum::extract::FromRef\">FromRef</a>&lt;OuterState&gt;,\n    OuterState: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Sync.html\" title=\"trait core::marker::Sync\">Sync</a>,</span>"],["impl&lt;T, S&gt; <a class=\"trait\" href=\"axum/extract/trait.FromRequestParts.html\" title=\"trait axum::extract::FromRequestParts\">FromRequestParts</a>&lt;S&gt; for <a class=\"struct\" href=\"axum/struct.Extension.html\" title=\"struct axum::Extension\">Extension</a>&lt;T&gt;<span class=\"where fmt-newline\">where\n    T: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/clone/trait.Clone.html\" title=\"trait core::clone::Clone\">Clone</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Sync.html\" title=\"trait core::marker::Sync\">Sync</a> + 'static,\n    S: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Sync.html\" title=\"trait core::marker::Sync\">Sync</a>,</span>"],["impl&lt;T, S&gt; <a class=\"trait\" href=\"axum/extract/trait.FromRequestParts.html\" title=\"trait axum::extract::FromRequestParts\">FromRequestParts</a>&lt;S&gt; for <a class=\"struct\" href=\"axum/extract/struct.Path.html\" title=\"struct axum::extract::Path\">Path</a>&lt;T&gt;<span class=\"where fmt-newline\">where\n    T: <a class=\"trait\" href=\"serde/de/trait.DeserializeOwned.html\" title=\"trait serde::de::DeserializeOwned\">DeserializeOwned</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a>,\n    S: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Sync.html\" title=\"trait core::marker::Sync\">Sync</a>,</span>"],["impl&lt;S&gt; <a class=\"trait\" href=\"axum/extract/trait.FromRequestParts.html\" title=\"trait axum::extract::FromRequestParts\">FromRequestParts</a>&lt;S&gt; for <a class=\"struct\" href=\"axum/extract/struct.Host.html\" title=\"struct axum::extract::Host\">Host</a><span class=\"where fmt-newline\">where\n    S: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Sync.html\" title=\"trait core::marker::Sync\">Sync</a>,</span>"],["impl&lt;T, S&gt; <a class=\"trait\" href=\"axum/extract/trait.FromRequestParts.html\" title=\"trait axum::extract::FromRequestParts\">FromRequestParts</a>&lt;S&gt; for <a class=\"struct\" href=\"axum/extract/struct.Query.html\" title=\"struct axum::extract::Query\">Query</a>&lt;T&gt;<span class=\"where fmt-newline\">where\n    T: <a class=\"trait\" href=\"serde/de/trait.DeserializeOwned.html\" title=\"trait serde::de::DeserializeOwned\">DeserializeOwned</a>,\n    S: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Sync.html\" title=\"trait core::marker::Sync\">Sync</a>,</span>"],["impl&lt;S&gt; <a class=\"trait\" href=\"axum/extract/trait.FromRequestParts.html\" title=\"trait axum::extract::FromRequestParts\">FromRequestParts</a>&lt;S&gt; for <a class=\"struct\" href=\"axum/extract/struct.RawPathParams.html\" title=\"struct axum::extract::RawPathParams\">RawPathParams</a><span class=\"where fmt-newline\">where\n    S: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Sync.html\" title=\"trait core::marker::Sync\">Sync</a>,</span>"],["impl&lt;S&gt; <a class=\"trait\" href=\"axum/extract/trait.FromRequestParts.html\" title=\"trait axum::extract::FromRequestParts\">FromRequestParts</a>&lt;S&gt; for <a class=\"struct\" href=\"axum/extract/struct.RawQuery.html\" title=\"struct axum::extract::RawQuery\">RawQuery</a><span class=\"where fmt-newline\">where\n    S: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Sync.html\" title=\"trait core::marker::Sync\">Sync</a>,</span>"],["impl&lt;S&gt; <a class=\"trait\" href=\"axum/extract/trait.FromRequestParts.html\" title=\"trait axum::extract::FromRequestParts\">FromRequestParts</a>&lt;S&gt; for <a class=\"struct\" href=\"axum/extract/struct.MatchedPath.html\" title=\"struct axum::extract::MatchedPath\">MatchedPath</a><span class=\"where fmt-newline\">where\n    S: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.74.0/core/marker/trait.Sync.html\" title=\"trait core::marker::Sync\">Sync</a>,</span>"]],
"axum_core":[]
};if (window.register_implementors) {window.register_implementors(implementors);} else {window.pending_implementors = implementors;}})()