export {
  search as searchPlugin,
  SearchQuery,
  getSearchState,
  setSearchState,
  getMatchHighlights,
  findNext as searchFindNext,
  findPrev as searchFindPrev,
  replaceAll as searchReplaceAll,
  replaceCurrent as searchReplaceCurrent,
  replaceNext as searchReplaceNext,
} from "prosemirror-search";
import "prosemirror-search/style/search.css";
