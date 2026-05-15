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

extension Notification.Name {
    /// Posted when a bearer-authenticated API request returns 401, signalling
    /// that the stored session token is no longer valid and the user must
    /// sign in again.
    static let ramekinAuthExpired = Notification.Name("ramekinAuthExpired")
}

private func notifyIfAuthExpired<T>(_ result: Swift.Result<Response<T>, ErrorResponse>) {
    guard case let .failure(.error(statusCode, _, _, _)) = result, statusCode == 401 else {
        return
    }
    NotificationCenter.default.post(name: .ramekinAuthExpired, object: nil)
}

/// Request builder that accepts self-signed certificates.
class InsecureRequestBuilder<T>: URLSessionRequestBuilder<T> {
    override func createURLSession() -> URLSessionProtocol { insecureSession }

    @discardableResult
    override func execute(
        _ apiResponseQueue: DispatchQueue = RamekinClientAPI.apiResponseQueue,
        _ completion: @escaping (_ result: Swift.Result<Response<T>, ErrorResponse>) -> Void
    ) -> RequestTask {
        super.execute(apiResponseQueue) { result in
            notifyIfAuthExpired(result)
            completion(result)
        }
    }
}

/// Decodable request builder that accepts self-signed certificates.
class InsecureDecodableBuilder<T: Decodable>: URLSessionDecodableRequestBuilder<T> {
    override func createURLSession() -> URLSessionProtocol { insecureSession }

    @discardableResult
    override func execute(
        _ apiResponseQueue: DispatchQueue = RamekinClientAPI.apiResponseQueue,
        _ completion: @escaping (_ result: Swift.Result<Response<T>, ErrorResponse>) -> Void
    ) -> RequestTask {
        super.execute(apiResponseQueue) { result in
            notifyIfAuthExpired(result)
            completion(result)
        }
    }
}

/// Factory for insecure request builders.
class InsecureBuilderFactory: RequestBuilderFactory {
    func getNonDecodableBuilder<T>() -> RequestBuilder<T>.Type { InsecureRequestBuilder<T>.self }
    func getBuilder<T: Decodable>() -> RequestBuilder<T>.Type { InsecureDecodableBuilder<T>.self }
}
