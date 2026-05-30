/// Thrown when the native search engine reports an error.
class SearchException implements Exception {
  final String message;

  const SearchException(this.message);

  @override
  String toString() => 'SearchException: $message';
}
