import Foundation

/// URLSession delegate that accepts self-signed certificates for development.
class InsecureSessionDelegate: NSObject, URLSessionDelegate, URLSessionTaskDelegate {
    func urlSession(
        _ session: URLSession,
        didReceive challenge: URLAuthenticationChallenge,
        completionHandler: @escaping (URLSession.AuthChallengeDisposition, URLCredential?) -> Void
    ) {
        acceptChallenge(challenge, completionHandler: completionHandler)
    }

    func urlSession(
        _ session: URLSession,
        task: URLSessionTask,
        didReceive challenge: URLAuthenticationChallenge,
        completionHandler: @escaping (URLSession.AuthChallengeDisposition, URLCredential?) -> Void
    ) {
        acceptChallenge(challenge, completionHandler: completionHandler)
    }

    private func acceptChallenge(
        _ challenge: URLAuthenticationChallenge,
        completionHandler: @escaping (URLSession.AuthChallengeDisposition, URLCredential?) -> Void
    ) {
        if let serverTrust = challenge.protectionSpace.serverTrust {
            completionHandler(.useCredential, URLCredential(trust: serverTrust))
        } else {
            completionHandler(.performDefaultHandling, nil)
        }
    }
}

/// Shared insecure URLSession for development.
let insecureSession: URLSession = {
    URLSession(configuration: .default, delegate: InsecureSessionDelegate(), delegateQueue: nil)
}()

/// Report a 401 on a request that carried `authorization` (the builder's
/// `Authorization` header) so only the session that sent it ends.
private func notifyIfAuthExpired<T>(
    _ result: Swift.Result<Response<T>, ErrorResponse>,
    authorization: String?
) {
    guard case let .failure(.error(statusCode, _, _, _)) = result else { return }
    AuthExpiry.notifyIfRejected(
        statusCode: statusCode,
        sentToken: AuthExpiry.bearerToken(fromAuthorizationHeader: authorization)
    )
}

/// Request builder that accepts self-signed certificates.
class InsecureRequestBuilder<T>: URLSessionRequestBuilder<T> {
    override func createURLSession() -> URLSessionProtocol { insecureSession }

    @discardableResult
    override func execute(
        _ apiResponseQueue: DispatchQueue = RamekinClientAPI.apiResponseQueue,
        _ completion: @escaping (_ result: Swift.Result<Response<T>, ErrorResponse>) -> Void
    ) -> RequestTask {
        let authorization = headers["Authorization"]
        return super.execute(apiResponseQueue) { result in
            notifyIfAuthExpired(result, authorization: authorization)
            completion(result)
        }
    }
}

/// Decodable request builder that accepts self-signed certificates.
class InsecureDecodableBuilder<T: Decodable>: URLSessionDecodableRequestBuilder<T> {
    override func createURLSession() -> URLSessionProtocol { insecureSession }

    override func createURLRequest(
        urlSession: URLSessionProtocol,
        method: HTTPMethod,
        encoding: ParameterEncoding,
        headers: [String: String]
    ) throws -> URLRequest {
        var request = try super.createURLRequest(
            urlSession: urlSession, method: method, encoding: encoding, headers: headers
        )
        // Text review waits for extraction plus the default import normalize ops.
        if request.url?.path == "/api/import/text" {
            request.timeoutInterval = 180
        }
        return request
    }

    @discardableResult
    override func execute(
        _ apiResponseQueue: DispatchQueue = RamekinClientAPI.apiResponseQueue,
        _ completion: @escaping (_ result: Swift.Result<Response<T>, ErrorResponse>) -> Void
    ) -> RequestTask {
        let authorization = headers["Authorization"]
        return super.execute(apiResponseQueue) { result in
            notifyIfAuthExpired(result, authorization: authorization)
            completion(result)
        }
    }
}

/// Factory for insecure request builders.
class InsecureBuilderFactory: RequestBuilderFactory {
    func getNonDecodableBuilder<T>() -> RequestBuilder<T>.Type { InsecureRequestBuilder<T>.self }
    func getBuilder<T: Decodable>() -> RequestBuilder<T>.Type { InsecureDecodableBuilder<T>.self }
}
