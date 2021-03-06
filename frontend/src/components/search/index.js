import { h, Component } from 'preact';
import style from './style.less';
import Protocol from '../../lib/protocol.js';
import { Card } from 'preact-mdl';
import TrackList from '../tracklist';

// TODO advanced search method
// gets called when this route is navigated to
async function search(stream) {
    return tracks;
}

export default class Search extends Component {
    state = {
        updating: false,
        finished: false,
        stream: null,
        query: null
    };

    update = (props) => {
        if(this.state.query != props.query) {
            let stream = Protocol.search(props.query);
            this.setState({ stream: stream, query: props.query, tracks: [], updating: false, finished: false }, e => {this.more()})
        }
    }

    more = async () => {
        if(this.state.updating || this.state.finished)
            return;

        this.setState({ updating: true });

        let tracks = await Promise.all(Array(20).fill(0).map(_ => this.state.stream.next()));

        const length = tracks.filter(x => !x.done).length;
        tracks = tracks.slice(0, length);
        tracks = tracks.map(x => x.value);

        const tmp = this.state.tracks.slice();
        tmp.push.apply(tmp, tracks);

        if(length < 20) 
            this.setState({ tracks: tmp, finished: true, updating: false });
        else
            this.setState({ tracks: tmp, updating: false});
    }

    componentDidMount() {
        this.update(this.props);

    }

    componentWillReceiveProps(props) {
        this.update(props);
    }

	// Note: `user` comes from the URL, courtesy of our router
	render({}, { query, updating, stream, tracks }) {
        if(stream && tracks.length > 0) {
            return (
                <TrackList loadMore={this.more} tracks={tracks} />
            );
        } else {
            return (
                <div class={style.search}>Nothing found</div>
            );
        }
	}
}
